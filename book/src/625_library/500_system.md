# System

These modules are thin. Most are a named wrapper over one intrinsic, and the
value is the name: `std::os::env("HOME")` says what it does where
`.env_var("HOME")` says where it comes from. Prefer the wrappers — they are
where shared behaviour accumulates, and `std::fs`'s streaming decoder is already
an example of a wrapper doing real work.

## `std::io` — console

```fol
fun[exp] write(value: str): str
fun[exp] write_err(value: str): str
fun[exp] read_key(): int
fun[exp] read_key_ms(timeout: int): int
fun[exp] echo_int(value: int): int
fun[exp] echo_str(value: str): str
fun[exp] echo_bool(value: bol): bol
fun[exp] echo_chr(value: chr): chr
```

`write` adds no newline and flushes, and returns its argument unchanged so it
can sit inside an expression. The `echo_*` family prints a value in its FOL
form and forwards it, which suits tracing.

`read_key` blocks for one byte and yields `-1` at end of input. `read_key_ms`
gives up after the timeout, yielding `-2` — a distinct value, so a timeout is
not mistaken for a closed stream.

## `std::fs` — files

Whole-file access:

```fol
fun[exp] read_file(path: str): str
fun[exp] write_file(path: str, contents: str): int
fun[exp] dir_list(path: str): str
```

`read_file` returns `""` for a missing file, an empty file, **and** a file that
is not valid UTF-8. When those need telling apart, or the file is not text, read
it as bytes with the `.read_bytes(...)` intrinsic and convert with
`.str_from_bytes(...)`.

### Streaming

A file larger than memory, or one still being written, cannot be handled by the
whole-file routines at all.

```fol
con[exp] MODE_READ: int = 0;
con[exp] MODE_WRITE: int = 1;
con[exp] MODE_APPEND: int = 2;
con[exp] MODE_UPDATE: int = 3;

con[exp] FROM_START: int = 0;
con[exp] FROM_HERE: int = 1;
con[exp] FROM_END: int = 2;

fun[exp] open_read(path: str): int
fun[exp] open_write(path: str): int
fun[exp] open_append(path: str): int
fun[exp] open_update(path: str): int
fun[exp] read_chunk(handle: int, count: int): vec[int]
fun[exp] write_chunk(handle: int, bytes: vec[int]): int
fun[exp] write_text(handle: int, text: str): int
fun[exp] seek(handle: int, offset: int, whence: int): int
fun[exp] rewind(handle: int): int
fun[exp] flush(handle: int): int
fun[exp] close(handle: int): int
```

A handle is an integer. The `open_*` routines exist because the underlying
`.file_open(path, mode)` takes an integer mode — FOL types a one-character
double-quoted literal as `chr`, so `"r"` cannot be passed as a `str`.

Reads and writes are **bytes**, which keeps the surface binary-safe.

### Decoding a chunked read

This is the trap, and it is worth stating plainly. A fixed-size read splits
multi-byte characters across chunk boundaries, and a chunk that ends
mid-character is not valid UTF-8 — so converting each chunk on its own **silently
loses those characters**. Reading `héllo` two bytes at a time yields `lo`.

`Decoder` holds the partial character until the rest of it arrives:

```fol
typ[exp] Decoder: rec = { carry: vec[int], text: str };

fun[exp] decoder(): Decoder
fun[exp] feed(state: Decoder, chunk: vec[int]): Decoder
fun[exp] read_streamed(path: str, chunk: int): str
```

`feed` returns the new state, whose `text` is what became decodable from that
chunk. `read_streamed` wraps the whole pattern and is lossless at every chunk
size:

```fol
var whole: str = std::fs::read_streamed(path, 4096);
```

Use it in preference to hand-rolling the loop.

## `std::os` — process and environment

```fol
fun[exp] env(name: str): str
fun[exp] shell(command: str): int
fun[exp] arg_count(): int
fun[exp] arg(index: int): str
```

`arg` excludes the program name, and an index past the end reads as `""`.

`shell` runs through `sh -c` and returns the exit status: `128 + signal` when a
signal killed it, `127` when the shell could not start. Because it is a shell,
the command is subject to quoting and word-splitting — when the arguments come
from user input, use the `.run_capture(...)` or `.run_status(...)` intrinsics
instead, which execute a program directly with no shell in between.

## `std::path` — path surgery

Pure string manipulation. It never touches the filesystem, so it works on paths
that do not exist yet.

```fol
con SEP: str = "/";

fun[exp] is_absolute(value: str): bol
fun[exp] join(left: str, right: str): str
fun[exp] file_name(value: str): str
fun[exp] parent(value: str): str
fun[exp] extension(value: str): str
fun[exp] file_stem(value: str): str
fun[exp] with_extension(value: str, ext: str): str
fun[exp] components(value: str): vec[str]
fun[exp] normalize(value: str): str
```

`normalize` resolves `.` and `..` textually. That is not the same as resolving
symlinks — a `..` after a symlink means something different on disk — so use it
for tidying displayed paths, not for security decisions.

`extension` returns `""` when there is no dot, and `file_stem` is the file name
without it.

## `std::term` — terminal

```fol
fun[exp] raw_mode(enable: bol): bol
fun[exp] cols(): int
fun[exp] rows(): int
```

`cols` and `rows` report 80×24 when the size cannot be determined, so a
redirected program still gets usable numbers.

Column counts are **not** string lengths: pad with `.str_width(...)`, since
`日本` occupies four columns while `.len(...)` reports six bytes.

## `std::time` — clock

```fol
fun[exp] now_ms(): int
fun[exp] sleep_ms(ms: int): int
```

`now_ms` is wall-clock, so it can jump backwards when the system clock is
adjusted. To measure a duration use the `.mono_ns()` intrinsic, which cannot.

## `std::sync` — shared counters

```fol
typ[exp] Counter: rec = { handle: int };

fun[exp] counter(initial: int): Counter
fun[exp] read(slot: Counter): int
fun[exp] write(slot: Counter, value: int): int
fun[exp] add(slot: Counter, delta: int): int
fun[exp] next(slot: Counter): int
fun[exp] compare_swap(slot: Counter, expected: int, desired: int): int
fun[exp] swapped(slot: Counter, expected: int, desired: int): bol
fun[exp] add_saturating(slot: Counter, delta: int, ceiling: int): int
fun[exp] cores(): int
fun[exp] this_thread(): int
fun[exp] relax(): int
```

A `Counter` is a **handle**, so copying it shares the count rather than
duplicating it — which is what lets one cross a `[spn]` boundary with no
reference threading:

```fol
var tally: std::sync::Counter = std::sync::counter(0);
[spn]worker::run(tally);
```

This is deliberately narrower than `mux[T]`. A mutex protects arbitrary compound
state and costs a lock; these compile to one instruction and cannot deadlock,
because there is no window in which the value is held. The moment two fields
have to agree, use `mux[T]`.

`add` and `next` return the value **before** the addition, so concurrent callers
each receive a distinct number — that is what makes `next` a ticket dispenser.
`compare_swap` returns the value it **found**: equal to `expected` means the swap
happened, anything else is the current value to retry against. `add_saturating`
shows the retry loop that is built from it.

`cores` is what a worker pool should size itself from; more threads than that
adds scheduling cost without adding throughput. Counters are never freed — they
live until the process exits, which suits program-wide tallies and not
per-request state.
