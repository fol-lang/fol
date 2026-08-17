# Intrinsics

Intrinsics are compiler-owned language operations.

They are not ordinary library functions, and they are not imported through
`use`.

FOL currently keeps compiler intrinsics and three API tiers separate:

- intrinsics:
  compiler-owned operations such as `.eq(...)`, `.len(...)`, `check(...)`, and
  `panic(...)`
- `core`:
  the minimal runtime model with no heap and no source-level hosted OS/runtime
  APIs
- `memo`:
  alloc-like heap-backed library/runtime support without source-level hosted
  OS/runtime APIs
- `std`:
  the hosted API tier layered on `memo` by an explicit bundled internal
  dependency, with shipped services such as console I/O

This split is not a source-level import trick and not an object-system feature.
`core` and `memo` are artifact capability models selected through `fol_model`
in `build.fol`. Bundled `std` is not a third model; it is declared separately
with `build.add_dep({ alias = "std", source = "internal", target = "standard" })`
for a `memo` artifact that needs hosted source APIs.

Whether an artifact can be launched is orthogonal. Host-compatible `core` and
`memo` programs can use `fol code run` or `fol code test` without bundled
`std`; the frontend launching them does not expose extra intrinsics.

If an operation can live as an ordinary library API, that is usually the better
home for it. Intrinsics are reserved for surfaces the compiler must understand
directly.

## Surfaces

The current compiler recognizes three intrinsic surfaces.

## Dot-root calls

These are written with a leading dot:

```fol
.eq(a, b)
.not(flag)
.len(items)
.echo(value)
```

Dot-root intrinsics are the main current intrinsic family.

## Keyword calls

These look like language keywords rather than dot calls:

```fol
check(read_code(path))
panic "unreachable state"
assert count > 0, "the queue was drained twice"
```

The current compiler treats `check`, `panic`, and `assert` as intrinsics too,
even though they are not written with `.`.

`panic` and `assert` take their arguments without parentheses, like the
keywords they are. `assert(flag)` happens to parse only because `(flag)` is a
parenthesized expression; `assert(flag, "message")` is a parse error.

## Operator aliases

Some future intrinsic surfaces are written like operators:

```fol
value as target_type
value cast target_type
```

These are registry-owned now, but they are not implemented in the current `V1`
compiler.

## Current `V1` implemented intrinsics

The current compiler implements this subset end to end through type checking and
lowering.

For current `V1`, backend execution of the implemented intrinsic set still goes
through the current runtime layer where policy matters. The runtime contract is
split by the artifact's `fol_model` and active bundled dependency, so the rule
is:

- `core` artifacts must not rely on heap-backed or source-level hosted APIs
- `memo` artifacts may use heap-backed facilities but not source-level hosted
  APIs
- bundled `std` wrappers require a `memo` artifact plus explicit internal
  `standard` dependency

All three API tiers may back executable artifacts. Process entry and
recoverable outcome adaptation are backend-only support, not bundled-std
intrinsics.

In the current implementation that means:

- `.len(...)` uses the runtime length helper
- `.echo(...)` uses the runtime echo hook and formatting contract
- `check(...)` uses the runtime recoverable-result inspection contract
- scalar comparisons and `.not(...)` may lower to native target operations

### Comparison

```fol
.eq(left, right)
.nq(left, right)
.lt(left, right)
.gt(left, right)
.ge(left, right)
.le(left, right)
```

Current `V1` rule:

- `.eq(...)` and `.nq(...)` work on comparable scalar pairs
- `.lt(...)`, `.gt(...)`, `.ge(...)`, and `.le(...)` work on ordered scalar
  pairs

If you call them with the wrong number of arguments or with unsupported type
families, the compiler reports an intrinsic-specific type error.

### Boolean

```fol
.not(flag)
```

Current `V1` rule:

- `.not(...)` accepts exactly one `bol`

### Query

```fol
.len(items)
```

Current `V1` rule:

- `.len(...)` accepts exactly one operand
- the operand must currently be one of:
  - `str`
  - `arr[...]`
  - `vec[...]`
  - `seq[...]`
  - `set[...]`
  - `map[...]`

`.len(...)` measures **bytes** for a `str`, not characters. `.str_char_len(...)`
is the character count; see "Text and characters" below for why the two worlds
are kept apart.

Under the runtime model split, array `.len(...)` belongs to `core`, while
string and dynamic-container `.len(...)` belongs to `memo`. It remains
available when a `memo` artifact also declares bundled `std` because the hosted
tier layers on top of `memo`; `.len(...)` does not itself require `std`.

### Diagnostic

```fol
.echo(value)
```

Current `V1` rule:

- `.echo(...)` accepts exactly one argument
- it requires a `memo` artifact with the explicit bundled `std` dependency
- it emits the value through the hosted runtime hook
- it then forwards the same value unchanged

`.echo(...)` belongs to `std`, not `core` or `memo`.

### Introspection

```fol
.type_name(value)
.size_of(value)
```

Both read a fact about the operand's **type** and never read the value, so they
accept an operand of any type — including a generic parameter, which is what
they exist for:

```fol
fun[exp] describe(T)(value: T): str = {
    return .type_name(value) + " (" + .int_to_str(.size_of(value)) + " bytes)";
};
```

Called with an `int` that yields `int (8 bytes)`; called with a `vec[int]`,
`vec[int] (24 bytes)`. The answer follows the caller, not the declaration.

Current rule:

- `.type_name(...)` returns the FOL spelling: `int`, `vec[int]`,
  `map[str, int]`, `opt[int]`, and the declared name for a `rec` or `ent` type
- `.size_of(...)` returns the size of the **value**, not of what it owns — a
  `str` and a `vec[int]` both report `24`, the size of the handle, because the
  characters and elements live on the heap
- both peel `@` and `bor` first, so a loaned `Ledger` describes as `Ledger` and
  measures the Ledger rather than the loan; `ptr[...]` is *not* peeled, because
  a pointer is a value you hold
- `.size_of(...)` works in `core`; `.type_name(...)` returns `str` and so needs
  `memo`, by virtue of its return type rather than any capability gate

### Terminal and OS hooks

Everything from here down shares `.echo(...)`'s build contract: a `memo`
artifact with the explicit bundled `std` dependency. That includes the purely
numeric operations such as `.bit_and(...)` and `.sqrt(...)`, which touch no OS
facility but are still gated this way today.

With that build contract, this is valid:

```fol
fun[] main(flag: bol): bol = {
    return .echo(flag);
};
```

#### How failure is reported

A hosted call reports failure with a sentinel, not a recoverable error: **`-1`
for an integer result, and the empty string for a text one.** That is uniform —
there is no routine here that signals failure with `1`.

A sentinel says *that* something went wrong and nothing about *what*, so the
reason is fetched afterwards, the way `errno` works:

- `.os_error()` — the reason as text, or `""` when the last hosted call
  succeeded
- `.os_error_kind()` — a stable integer code, or `0` on success

```fol
var text: str = .read_file(path);
if (.len(text) == 0) {
    .write("cannot read " + path + ": " + .os_error() + "\n");
} else {
};
```

Both are **cleared by a successful call**, so a stale reason is never read as a
fresh one, and both are **per-thread**, so two threads failing at once do not
overwrite each other.

Branch on the code rather than the message — the text is the operating system's
wording and is not a contract. `std::os` exports the codes by name
(`ERR_NOT_FOUND`, `ERR_DENIED`, …) along with `not_found()` and `denied()`:

```text
 0  none            8  address in use
 1  not found       9  address unavailable
 2  denied         10  broken pipe
 3  exists         11  would block
 4  refused        12  timed out
 5  reset          13  invalid input
 6  aborted        14  invalid data
 7  not connected  15  unexpected end of file
                   16  interrupted
                   17  wrote zero
                   99  anything else
```

The numbers are part of the surface and will not be renumbered. `99` absorbs
every other cause, including kinds Rust has not stabilised, so a toolchain
upgrade cannot silently change an existing code.

This is what makes `.read_file(...)` usable: it returns `""` for a missing file,
an empty file, **and** one that is not valid UTF-8, and the error kind is what
tells those apart.

#### Console and terminal

- `.write(text)` — write a string to stdout without a trailing newline and
  flush it; forwards the string unchanged
- `.write_err(text)` — write to standard error without a trailing newline
- `.read_key()` — block for one byte of standard input; yields `-1` at end of
  input
- `.read_key_ms(timeout)` — like `.read_key()` but gives up after the timeout,
  yielding `-2` on timeout and `-1` at end of input
- `.read_line()` / `.read_all()` — a line, or the whole of standard input
- `.raw_mode(enable)` — enable or disable terminal raw mode; forwards the
  requested state
- `.term_cols()` / `.term_rows()` — terminal size (80×24 when it cannot be
  determined)

#### Numbers

Integer helpers:

- `.min(a, b)` / `.max(a, b)` / `.abs(value)`
- `.parse_int(text, fallback)` — parse an integer, or the caller's fallback; the
  fallback is an argument because every sentinel is also a valid parse result

Bitwise, on non-negative integers:

- `.bit_and(a, b)` / `.bit_or(a, b)` / `.bit_xor(a, b)`
- `.shl(value, count)` / `.shr(value, count)` — `.shr(...)` is arithmetic, so a
  negative value shifts sign bits in
- `.rotl(value, count)` / `.rotr(value, count)`
- `.pop_count(value)` / `.clz(value)` / `.ctz(value)` — set bits, leading
  zeros, trailing zeros

Overflow-mode arithmetic. Plain `+`, `-`, and `*` fault on overflow; these are
how you choose a different answer:

- `.checked_add(a, b)` / `.checked_sub(a, b)` / `.checked_mul(a, b)` /
  `.checked_div(a, b)`
- `.wrapping_add(a, b)` / `.wrapping_sub(a, b)` / `.wrapping_mul(a, b)`
- `.saturating_add(a, b)` / `.saturating_sub(a, b)` / `.saturating_mul(a, b)`

Floating point:

- `.sqrt(value)`, `.flt_abs(value)`
- `.sin(value)`, `.cos(value)`, `.tan(value)`, `.asin(value)`, `.acos(value)`,
  `.atan(value)`, `.atan2(y, x)`, `.hypot(x, y)`
- `.ln(value)`, `.log2(value)`, `.log10(value)`, `.exp(value)`
- `.is_nan(value)`, `.is_inf(value)`, `.flt_is_finite(value)`
- `.flt_floor(value)`, `.flt_ceil(value)`, `.flt_round(value)`
- `.flt_copysign(magnitude, sign)`, `.flt_rem(a, b)`,
  `.flt_mul_add(a, b, c)`, `.flt_next_after(from, toward)`
- `.flt_bits(value)` / `.flt_from_bits(bits)` — the IEEE-754 bit pattern, which
  is how you compare or hash a float exactly

Conversions:

- `.int_to_str(value)` / `.float_to_str(value, decimals)`
- `.flt_to_str_exact(value)` — the shortest text that parses back to the
  identical value. `.float_to_str(...)` takes a fixed number of decimals, which
  either loses precision or pads noise, so neither setting round-trips: `0.1`
  renders as `0.100000` at six decimals but as `0.1` here. `.flt_bits(...)`
  remains the exact *machine* form; this is the exact human-readable one
- `.int_to_flt(value)` / `.flt_to_int(value)` / `.parse_flt(text, fallback)`

#### Text and characters

FOL strings are UTF-8, and the intrinsics are split into a **byte** world and a
**character** world. Mixing them is the usual source of bugs, so the names say
which one they belong to:

- byte world: `.len(text)`, `.str_sub(text, start, count)`,
  `.str_find(text, needle)`, `.str_byte(text, index)`
- character world: `.str_char_len(text)`, `.str_char(text, index)`,
  `.str_chars(text)`

`.str_char_index(text, char_index)` is the only bridge between them: it turns a
character index into a byte index.

There is a third measure that is neither: **terminal columns**. `日本` is 6
bytes, 2 characters, and 4 columns, so padding a table with `.len(...)` or
`.str_char_len(...)` misaligns it. `.str_width(text)` and `.chr_width(c)` are
what a column width has to be computed from.

- `.str_bytes(text)` / `.str_from_bytes(bytes)` — text to its UTF-8 bytes and
  back. `.str_from_bytes(...)` is the **only** way to rebuild text from
  `.read_bytes(...)`, `.random_bytes(...)`, or `.file_read(...)`:
  `.byte_to_str(...)` handles one byte and so cannot express a multi-byte
  character at all. Invalid input yields the empty string rather than
  substituting replacement characters, so a caller never acts on
  half-decoded text
- `.bytes_valid_utf8(bytes)` — whether a byte vector decodes; worth asking when
  empty input and invalid input have to be told apart
- `.utf8_prefix_len(bytes)` — how many leading bytes form *complete* UTF-8.
  Required by any chunked reader: a fixed-size read splits characters across
  chunk boundaries, and a chunk ending mid-character decodes to the empty
  string. Decode the prefix, carry the remainder. `std::fs::decoder` wraps the
  whole pattern, and `std::fs::read_streamed` uses it
- `.str_normalize(text, form)` / `.str_is_normalized(text, form)` — see below
- `.str_width(text)` / `.chr_width(c)` — terminal columns: 0 for combining
  marks and controls, 2 for CJK, kana, Hangul, fullwidth forms and the common
  emoji planes, 1 otherwise. It is a range table, as `wcwidth` always is, so a
  rare combining mark outside the listed ranges counts as one column instead of
  zero

- `.str_sub(text, start, count)` — a byte-range slice snapped to UTF-8
  boundaries
- `.str_byte(text, index)` / `.byte_to_str(value)` — byte inspection and
  single-byte construction
- `.str_byte_len(text)` / `.str_char_len(text)` — length in bytes, length in
  characters
- `.str_valid_utf8(text)` — whether the bytes decode
- `.str_find(text, needle)` — the byte index of the first occurrence, or `-1`
- `.str_replace(text, from, to)` — replace every occurrence
- `.str_trim(text)`, `.str_upper(text)`, `.str_lower(text)`
- `.str_chars(text)` / `.str_from_chars(chars)` — a string as `vec[chr]` and
  back
- `.chr_upper(c)` / `.chr_lower(c)` — full Unicode case, not ASCII-only
- `.chr_is_alpha(c)` / `.chr_is_digit(c)` / `.chr_is_space(c)`
- `.chr_to_int(c)` / `.int_to_chr(value)` / `.chr_to_str(c)`

#### Normalization

The same word can be typed two ways. `é` as `e` plus a combining accent and `é`
precomposed look identical, compare **unequal**, and report different lengths:

```fol
var typed: str = "e\u{0301}llo";
var stored: str = "éllo";
// typed == stored          is false
// .str_char_len(typed)     is 5
// .str_char_len(stored)    is 4
// std::strn::same_text(typed, stored)  is true
```

Any comparison against text a person typed — a login name, a search box, a
filename — has to normalize both sides first, or it rejects input the user
believes is correct.

`.str_normalize(text, form)` takes a form selector:

- `0` **NFC** — compose. The default for storing and comparing
- `1` **NFD** — decompose. Useful for stripping accents, since it separates the
  base letter from its marks
- `2` **NFKC** — compose, folding compatibility variants first: `ﬁ` becomes
  `fi`, fullwidth `Ａ` becomes `A`. Lossy by design: right for a search key,
  wrong for text handed back to the user, because it discards distinctions the
  author made
- `3` **NFKD** — decompose with the same folding

An unknown form returns the input unchanged. `.str_is_normalized(text, form)`
answers without doing the work, which is worth it for stored text where the
answer is usually yes.

`std::strn` wraps these as `nfc`, `nfd`, `nfkc`, `nfkd`, `is_nfc`, plus
`same_text(left, right)` for comparing user input and `search_key(text)` for
duplicate detection.

The Unicode data behind this is **generated into the runtime**, not pulled from
a crate: the runtime is compiled by a bare `rustc` with no dependency
resolution, so it can only carry plain data. See
`fol-runtime/tools/README.md` to regenerate.

#### Time and randomness

- `.now_ms()` / `.now_ns()` — since the unix epoch; wall-clock, so it can jump
- `.mono_ns()` — a monotonic reading, which is the one to measure durations with
- `.sleep_ms(ms)` / `.sleep_ns(ns)`
- `.time_parts(epoch_secs)` — a timestamp split into calendar fields, in **UTC**
- `.time_from_parts(fields)` — the inverse
- `.tz_offset_sec(epoch_secs)` — seconds to add to UTC for local time. Since
  `.time_parts(...)` is UTC-only, without this every timestamp a program shows
  its reader is wrong

  It takes the **instant** because the offset is not a constant: the same zone is
  `3600` in January and `7200` in July. The whole zone database applies,
  including the daylight-saving rule in force at that moment, which is the part
  a hand-rolled reader gets wrong. `std::time::local_parts(...)` combines the two
- `.random_int(low, high)` / `.random_flt()` — from the operating system, so
  unpredictable and unrepeatable
- `.random_bytes(count)` — raw entropy

##### Two kinds of randomness

The intrinsics above take entropy from the operating system, so they are
unpredictable *and* unrepeatable. That second property is often the problem: a
test, a simulation, or procedural generation needs to produce the same result
twice.

`std::rand` is the other tool — seeded, repeatable, and written in FOL over the
bitwise intrinsics rather than reaching the OS at all:

```fol
var[mut] rng: std::rand::Rng = std::rand::seeded(2024);
var roll: std::rand::Draw = std::rand::next_range(rng, 1, 7);
// roll.value is the die; roll.rng is the advanced generator
```

The generator is a pure routine, so it returns the advanced state alongside the
value and the caller threads it along. Seed from `std::rand::from_os()` when you
want a different stream each run but still want to be able to log the seed and
reproduce a failure.

The algorithm is **xorshift64\***: Vigna's 12/25/27 shift triple over a full
64-bit state, with the output passed through a multiplicative scrambler.

That final multiply is not decoration. A plain xorshift is *linear over GF(2)* —
every output is a linear function of the state — so its outputs are far more
predictable than their frequency histogram suggests. Reading 96-bit vectors out
of the two generators and taking the rank of a 96×96 binary matrix shows it
plainly:

```text
plain xorshift, no scrambler:  rank 46 of 96   (on every seed)
xorshift64* as shipped:        rank 95 of 96   (on every seed)
```

95 is the expected rank for a genuinely random binary matrix. A `%`-reduction
test — die rolls, bucket indices, coin flips — cannot see the difference at
all; both look uniform. This is why the scrambler is there.

It is still **not cryptography**. The state is recoverable from the output,
which is precisely what makes it repeatable. When unpredictability matters, use
`.random_bytes(...)`.

Seeds are ordinary 64-bit integers, negatives included. Only zero is special:
it is a fixed point that would emit zeros forever, so it is replaced.

#### Files and directories

Whole-file access:

- `.read_file(path)` / `.write_file(path, contents)` / `.append_file(path, text)`
- `.read_bytes(path)` / `.write_bytes(path, bytes)` — for content that is not
  UTF-8; the text forms assume it is
- `.file_exists(path)`, `.is_file(path)`, `.is_dir(path)`,
  `.file_is_link(path)`
- `.file_size(path)`, `.file_mtime(path)`
- `.make_dir(path)`, `.remove_file(path)`, `.remove_dir_all(path)`
- `.rename_file(from, to)`, `.copy_file(from, to)`
- `.dir_list(path)` — the sorted entries, directories suffixed with `/`
- `.dir_entries(path)` — the same as a vector
- `.read_link(path)`, `.make_symlink(target, link)` — follow and create. Creating
  was missing, which left any program managing a `current -> release` symlink
  unwritable
- `.permissions(path)`, `.set_permissions(path, mode)`
- `.current_dir()`, `.set_current_dir(path)`, `.temp_dir()`, `.home_dir()`
- `.realpath(path)` — the absolute path with symlinks and `..` resolved **on
  disk**. `std::path::normalize` resolves `..` textually, which is a different
  answer whenever a symlink is involved, so only this one can decide whether a
  path stays inside an allowed directory. The path must already exist
- `.temp_file(prefix)` — creates a uniquely named empty file in the temp
  directory and returns its path. The *create* is the point: choosing a name in
  FOL and then opening it would race, because another process could take the
  name in between. With `.rename_file(...)`, this is the safe-write pattern —
  write a temp file, then rename it over the target so a reader never sees a
  half-written one
- `.file_lock(handle, exclusive, wait)` / `.file_unlock(handle)` — advisory
  whole-file locking, which is how a program stays single-instance: take an
  exclusive lock on a known path and exit if another process holds it. `wait`
  false returns `-1` immediately instead of blocking, so a caller can report
  "already running" rather than hang. Advisory means it binds only processes
  that also ask; it does not stop an unrelated writer

Streaming, for a file larger than memory or one still being written — the
whole-file forms cannot do either:

- `.file_open(path, mode)` — mode `0` read, `1` truncate-or-create, `2` append,
  `3` read-write. An integer rather than `"r"`/`"w"`, because FOL types a
  one-character double-quoted literal as `chr`, so `.file_open(path, "r")` does
  not typecheck. `std::fs` exports `MODE_READ`…`MODE_UPDATE` and
  `open_read`/`open_write`/`open_append`/`open_update` so nobody has to
  remember the numbers
- `.file_read(handle, count)` — up to `count` **bytes**; an empty result means
  end of file
- `.file_write(handle, bytes)` — the byte count written, or `-1`
- `.file_seek(handle, offset, whence)` — `whence` `0` start, `1` current, `2`
  end; returns the new absolute position
- `.file_flush(handle)` / `.file_close(handle)`

Reads and writes are bytes, not text, so the surface is binary-safe;
`.str_bytes(...)` and `.str_from_bytes(...)` bridge to text. **Do not feed a
raw chunk to `.str_from_bytes(...)`** — see `.utf8_prefix_len(...)` above, and
prefer `std::fs::read_streamed(path, chunk)`, which is lossless at every chunk
size.

#### Process and environment

- `.arg_count()` / `.arg_at(index)` — the command-line arguments, excluding the
  program name; an index past the end reads as the empty string
- `.arg_program()` — argv[0], the path this program was invoked as. `arg_at(0)`
  is the FIRST argument, so this is what a usage message should name and what a
  program re-executes
- `.env_var(name)`, `.env_vars()`, `.set_env_var(name, value)`
- `.unset_env_var(name)` — removing is not the same as setting to `""`. An empty
  value is still a *present* variable, and a child process can tell the
  difference
- `.process_id()`
- `.shell(command)` — run through `sh -c` with inherited streams and yield the
  exit status; `128 + signal` if a signal killed it, `127` if the shell could
  not be launched
- `.shell_out(command)` — the same, capturing standard output
- `.run_capture(program, args)` / `.run_status(program, args)` — run a program
  directly, without a shell, so no quoting or word-splitting applies
- `.run_input(program, args, stdin)` — the same, with text on the child's
  standard input. `.run_capture(...)` gives the child a closed stdin, so every
  filter-shaped program (`sort`, `sha256sum`, `git hash-object --stdin`) needs
  this one
- `.exit_process(status)`

Supervised children. `.run_capture(...)`, `.run_status(...)` and
`.run_input(...)` all block until the child exits — right for a filter, useless
for anything you intend to watch and stop. These separate starting from waiting:

- `.child_spawn(program, args)` — start without waiting, yielding a handle.
  Standard streams are inherited, so the child shares this program's terminal;
  when the output is what you want, `.run_capture(...)` is still the answer
- `.child_pid(handle)` — the child's process id, for logging
- `.child_try_wait(handle)` — the exit status if it has finished, **`-2` while it
  is still running**, `-1` for an unknown handle. Three outcomes, so a poll loop
  cannot confuse "not yet" with "gone". A finished child **keeps** its status, so
  reading it twice gives the same answer and a later `.child_wait(...)` still
  works
- `.child_wait(handle)` — block until it exits, then release the handle so the
  child is never left as a zombie. Returns the kept status when a poll already
  saw it finish
- `.child_kill(handle, signum)` — `0` means `SIGKILL`, which cannot be caught;
  any other number is sent as given, so `15` asks politely and lets the child
  run its own cleanup

Signals:

- `.signal_trap(signum)` — record a signal instead of dying from it. Returns
  `0`, or `-1` for an out-of-range number or one the kernel refuses (`SIGKILL`
  and `SIGSTOP` cannot be caught)
- `.signal_pending()` — the lowest trapped signal that has arrived since the
  last call, or `0`. Reading it clears the flag, so each delivery is reported
  once

The surface is a poll rather than a callback on purpose: the handler itself can
only do async-signal-safe work, and running FOL code inside one would not be
safe. Trap the signal, then check for it at a point of your choosing — the top
of an event loop, or between units of work.

#### Network

TCP sockets are integer handles:

- `.tcp_listen(address)`, `.tcp_accept(handle)`, `.tcp_connect(address)`
- `.tcp_read(handle)`, `.tcp_write(handle, text)`, `.tcp_close(handle)`
- `.tcp_local_addr(handle)`, `.tcp_peer_addr(handle)`
- `.tcp_set_timeout(handle, ms)` — without it, a read on a peer that stopped
  talking blocks until the peer goes away
- `.tcp_set_nodelay(handle, enable)`
- `.tcp_try_read(handle)` — returns empty when nothing has arrived **yet**,
  which is not the same as the connection being closed; neither read form
  distinguishes those, so a poll loop needs its own liveness signal
- `.tcp_shutdown(handle, how)` — half-close: `0` read, `1` write, `2` both.
  This is not `.tcp_close(...)`; shutting down the write side tells the peer no
  more data is coming while the read side stays open for its reply

UDP and name resolution:

- `.udp_bind(address)`, `.udp_send_to(handle, address, text)`,
  `.udp_recv_from(handle)` — the receive form returns the payload and the
  sender's address
- `.dns_resolve(host)` — every address a name resolves to; an empty vector when
  it resolves to none, rather than a fault

Serving several connections at once:

- `.poll_read(handles, timeout_ms)` — which of the given socket handles have data
  ready, waiting up to `timeout_ms`; an empty result means the timeout expired,
  and a negative timeout waits indefinitely. Unknown handles are ignored rather
  than faulting

Without it, serving several connections means a thread per connection — which
FOL can do with `[spn]`, and which stops scaling once the connections outnumber
what the scheduler should carry. This is the other shape: one thread, many
sockets. A listener becomes ready when a connection is pending, so the same call
covers both accepting and reading.

#### Concurrency

- `.cpu_count()` — how many threads can genuinely run at once; sizing a worker
  pool above this adds scheduling cost, not throughput
- `.thread_yield()` — hand back the rest of the time slice
- `.thread_id()` — a small integer per thread, stable within a run

Atomic counters are shared integers addressed by handle. They are deliberately
narrower than `mux[T]`: a mutex protects arbitrary compound state and costs a
lock, while these compile to one instruction and cannot deadlock, because there
is no window in which the value is held. Reach for `mux[T]` the moment two
fields have to agree.

- `.atomic_new(initial)` — create one, yielding its handle
- `.atomic_load(handle)` / `.atomic_store(handle, value)`
- `.atomic_add(handle, delta)` — returns the value **before** the addition, so
  concurrent callers each receive a distinct number; that is what makes it a
  ticket dispenser
- `.atomic_cas(handle, expected, desired)` — swap if unchanged, returning the
  value it **found**. Equal to `expected` means the swap happened; anything
  else is the current value to retry against, which is what a lock-free update
  loop needs

Because a handle is an ordinary `int`, a counter crosses a `[spn]` boundary
with no reference threading. A missing handle is inert: a load reads `0` and a
store yields `-1`.

#### Hashing and secrets

- `.hash_bytes(text)` — a stable 64-bit SipHash-2-4. The same input gives the
  same number in every run and every build, so it is safe to persist or shard
  on, unlike the randomly-keyed hash behind `map[K, V]`. That stability is also
  the limit: the key is fixed, so it does **not** resist an attacker who
  chooses inputs to collide, and it is not a cryptographic digest. Use
  `std::hash` for SHA-256 when an adversary is involved. The result covers the
  full 64-bit range and is often negative.
- `.bytes_equal_ct(left, right)` — compare in time that depends only on length,
  never on where the two first differ. Ordinary `==` returns as soon as it
  finds a mismatching byte, so anyone who can time it learns how many leading
  bytes they guessed correctly and can recover a secret one byte at a time. A
  FOL loop compiles to the same early exit, which is why this cannot be written
  in FOL. Use it for tokens, MACs, and password hashes; use `==` everywhere
  else, since this is slower by design.

The bundled `std` package wraps much of this surface — `std::io::write`,
`std::fs::read_file`, `std::os::env`, `std::fmt::int_to_str`, `std::strn::find`,
`std::time::now_ms`, `std::sync::counter`, and so on. Prefer the wrappers: they
are named for what you are doing rather than for the primitive underneath, and
they are where the shared logic accumulates.

### The entry point's command line and exit status

An entry routine's parameters are bound, in order, to the command-line
arguments that follow the program name. Only the scalar builtins are bound
this way: `int`, `flt`, `bol`, `chr`, and `str`. `bol` accepts
`true`/`1`/`yes`/`on` and `false`/`0`/`no`/`off`.

A command line that does not satisfy the signature is a usage error, not a
default. A missing argument, or one that does not parse as the declared type,
writes a message naming the position and the expected type to standard error
and exits with status `2`. Nothing silently becomes `0` or `""`.

```fol
fun[] main(count: int, label: str): int = {
    return .echo(count);
};
```

```text
$ ./program notanumber hello
fol: command-line argument #1 is not a valid `int`: `notanumber`
$ echo $?
2
```

A parameter of any other type is not bound from the command line; it is
constructed empty. Read the command line yourself with `.arg_count()` and
`.arg_at(index)` when you need anything richer than a scalar.

The entry's `int` return value **is** the process exit status:

```fol
fun[] main(): int = {
    return 3;
};
```

```text
$ ./program; echo $?
3
```

This holds on both channels. A plain `fun[] main(): int` exits with the value
it returns, and a recoverable `fun[] main(): int / E` exits with the returned
value when it returns and with `1`, after printing the error, when it reports.
An entry declared `: non` always exits `0`. Exit statuses are truncated to
their low 8 bits by the operating system, so keep them in `0..=255`; `0` means
success by convention.

Because `main`'s return is the exit status, `return .echo(value)` makes the
echoed value the status too. Return `0` explicitly when a program succeeds.

### Recoverable and control intrinsics

```fol
check(read_code(path))
panic "fatal"
assert total >= 0;
assert total >= 0, "a ledger total went negative";
```

Current rule:

- `check(expr)` asks whether a recoverable `/ ErrorType` expression failed and
  returns `bol`; that expression may be a direct routine call or, in V3, an
  awaited recoverable eventual
- `panic(...)` aborts control flow immediately
- `assert condition` and `assert condition, message` fault unless the condition
  holds

`assert` is **not** a terminator the way `panic` is. Control continues when the
condition holds, so it is an ordinary effect rather than a diverging one; the
compiler does not treat the code after it as unreachable.

A failing assert produces a normal runtime fault:

```text
fol runtime fault: assertion failed: a ledger total went negative
```

The bare form works in every capability model. The message form takes a `str`
and so needs `memo`, since `str` does not exist below it.

`check` and `panic` are described in more detail in the recoverable-error
chapter.

### Backtraces

```fol
.backtrace()
```

Returns the call stack at that point as text, captured whether or not
`RUST_BACKTRACE` is set. Frames carry the generated symbol names rather than
FOL source spellings, and an optimized build may inline frames away entirely.
It is a debugging aid, not something to parse. Like the other hosted hooks it
needs a `memo` artifact with bundled `std`.

## Deferred intrinsics

The registry reserves more names than the compiler implements. A reserved name
is recognized as a registry-owned language surface and rejected with an
explicit milestone-boundary diagnostic — it does **not** work today.

Twenty-one names are currently in that state.

### Conversions

- `as`
- `cast`

The explicit conversion contract is not frozen. Use the named conversions —
`.int_to_flt(...)`, `.flt_to_int(...)`, `.int_to_str(...)`, `.parse_int(...)`
— which say in their name exactly which direction and which failure mode you
are asking for.

### Container queries

- `.cap(...)`
- `.is_empty(...)`
- `.low(...)`
- `.high(...)`

`.len(...) == 0` covers the common case of `.is_empty(...)` today.

### Arithmetic kept as candidates for the library

- `.add(...)`, `.sub(...)`, `.mul(...)`, `.div(...)`
- `.clamp(...)`
- `.floor(...)`, `.ceil(...)`, `.round(...)`, `.trunc(...)`
- `.pow(...)`

These stay reserved so the names cannot be claimed by user code, but the
direction is that most of them belong in a library rather than in the compiler.
The float-specific spellings `.flt_floor(...)`, `.flt_ceil(...)`, and
`.flt_round(...)` are implemented and are what to use now.

### Bitwise and overflow remainders

- `.byte_swap(...)`, `.bit_reverse(...)`
- `.overflowing_add(...)`, `.overflowing_sub(...)`

The rest of both families shipped. The `overflowing_*` pair returns a value
**and** a flag, which a record already expresses; that is why it was not
urgent.

### Reserved for `V4` interop

- `.de_alloc(...)`

Explicit deallocation is not part of the V3 memory model. Unique heap values
drop implicitly, borrowing uses `[bor]owner` and `[end]borrow`, and typed pointers use
`[ref]value` and `[drf]pointer`. The old dot-root memory spellings are not reserved or
supported aliases.

## Intrinsics are not shell operations

Do not confuse intrinsics with shell syntax such as `nil` and unwrap
`!`.

For example:

```fol
ali MaybeText: opt[str]
ali Failure: err[str]

fun[] unwrap_optional(value: MaybeText): str = {
    return [uwp]value
};

fun[] unwrap_failure(value: Failure): str = {
    return [uwp]value
};
```

That `!` surface is part of shell typing, not the intrinsic registry.

Likewise, recoverable routine calls such as:

```fol
fun[] read_code(path: str): int / str = { ... }
```

and V3 recoverable results produced by `eventual | await` are handled with:

- `check(expr)`
- `expr || fallback`

not with shell unwrap.

## Current compiler truth

The current compiler has one shared intrinsic registry crate:

`fol-intrinsics`

That registry is the source of truth for:

- canonical intrinsic names and aliases
- milestone availability (`V1` / `V2` / `V3`)
- type-checking selection rules
- lowering mode
- backend/runtime role classification

The current runtime companion for implemented `V1` intrinsics is:

`fol-runtime`

- intrinsic names
- aliases
- categories
- current milestone availability
- deferred-roadmap classification
- lowering mode
- backend-facing role

So the short rule is:

- parser recognizes intrinsic syntax
- `fol-intrinsics` owns intrinsic identity
- type checking validates intrinsic calls
- lowering maps them to explicit IR shapes

This page should describe only the subset that is actually implemented, plus
clearly marked deferred surfaces.
