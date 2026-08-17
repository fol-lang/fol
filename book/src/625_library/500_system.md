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
`.file_open(path, mode)` takes an integer mode, matching the selector arguments
elsewhere on that surface; these give the modes names.

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

### Paths, atomic writes, and locking

```fol
fun[exp] resolve(path: str): str
fun[exp] temp_path(prefix: str): str
fun[exp] write_atomic(path: str, contents: str): int
fun[exp] make_link(target: str, link: str): int
fun[exp] lock_exclusive(handle: int): int
fun[exp] try_lock_exclusive(handle: int): int
fun[exp] lock_shared(handle: int): int
fun[exp] unlock(handle: int): int
```

`resolve` asks the filesystem, so symlinks and `..` come out resolved as they
really are. `std::path::normalize` does it textually and is a different answer
whenever a symlink is involved — use `resolve` for any decision about whether a
path stays inside an allowed directory.

`write_atomic` writes a temp file and renames it over the target, so a reader
never sees a half-written file:

```fol
var wrote: int = std::fs::write_atomic("config.toml", rendered);
```

Rename is atomic only *within* a filesystem. `write_atomic` uses the temp
directory, so writing across a mount boundary falls back to a copy and loses the
guarantee; when that matters, make the temp file next to the target yourself with
`temp_path` and call `rename_file`.

`try_lock_exclusive` returns `-1` immediately instead of blocking, which is how a
program reports "already running" rather than hanging:

```fol
var handle: int = std::fs::open_update("/tmp/mytool.lock");
if (std::fs::try_lock_exclusive(handle) != 0) {
    std::io::write("another instance is running\n");
    return 1;
} else {
};
```

The lock is **advisory**: it binds only processes that also ask, and does not
stop an unrelated writer.

## `std::os` — errors, process, environment

### Why the last call failed

Every fallible routine in `std` reports failure as a sentinel — `-1`, or an
empty string — which says that something went wrong and nothing about what.
These read the reason afterwards:

```fol
fun[exp] error(): str
fun[exp] error_kind(): int
fun[exp] failed(): bol
fun[exp] not_found(): bol
fun[exp] denied(): bol
```

```fol
var text: str = std::fs::read_file(path);
if (std::os::not_found()) {
    std::io::write("no such file: " + path + "\n");
} else {
};
```

A successful call clears them, so a stale reason is never mistaken for a fresh
one. Prefer the predicates and the `ERR_*` constants over matching `error()`
text — the message is the operating system's wording, not a contract.

`ERR_NONE`, `ERR_NOT_FOUND`, `ERR_DENIED`, `ERR_EXISTS`, `ERR_REFUSED`,
`ERR_RESET`, `ERR_ABORTED`, `ERR_NOT_CONNECTED`, `ERR_ADDR_IN_USE`,
`ERR_ADDR_UNAVAILABLE`, `ERR_BROKEN_PIPE`, `ERR_WOULD_BLOCK`, `ERR_TIMED_OUT`,
`ERR_INVALID_INPUT`, `ERR_INVALID_DATA`, `ERR_UNEXPECTED_EOF`,
`ERR_INTERRUPTED`, `ERR_WRITE_ZERO`, `ERR_OTHER`.

### Arguments and environment

```fol
fun[exp] arg_count(): int
fun[exp] arg(index: int): str
fun[exp] program(): str
fun[exp] env(name: str): str
fun[exp] set_env(name: str, value: str): int
fun[exp] unset_env(name: str): int
fun[exp] shell(command: str): int
```

`arg` excludes the program name, and an index past the end reads as `""`.
`program()` is argv[0] — what a usage message should name.

`unset_env` is not `set_env(name, "")`: an empty value is still a present
variable, and a child can tell the difference.

### Supervised children

```fol
con[exp] SIGNAL_KILL: int = 0;
con[exp] SIGNAL_TERM: int = 15;

fun[exp] still_running(): int          // the -2 sentinel
fun[exp] spawn(program: str, args: vec[str]): int
fun[exp] child_id(handle: int): int
fun[exp] child_status(handle: int): int
fun[exp] child_running(handle: int): bol
fun[exp] await_child(handle: int): int
fun[exp] signal_child(handle: int, signum: int): int
```

`shell` and the `run_*` intrinsics block until the child finishes. These do not,
so a program can start something, watch it, and stop it:

```fol
var args: vec[str] = {"9000"};
var server: int = std::os::spawn("./serve", args);
loop (std::os::child_running(server)) {
    std::time::sleep_ms(100);
};
var status: int = std::os::await_child(server);
```

`child_status` returns three things — the exit status, `still_running()` while
it is going, or `-1` for an unknown handle — so a poll loop cannot confuse "not
yet" with "gone". A finished child keeps its status, which is what makes the loop
above safe: `await_child` after the poll returns the same value rather than
failing on a handle the poll consumed. `signal_child` with `SIGNAL_TERM` lets the child clean up;
`SIGNAL_KILL` cannot be caught.

`still_running()` is a routine rather than a constant because a global binding
needs a *literal* initializer and `-2` is a negation.

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
fun[exp] local_offset(epoch_secs: int): int
fun[exp] local_parts(epoch_secs: int): vec[int]
```

`now_ms` is wall-clock, so it can jump backwards when the system clock is
adjusted. To measure a duration use the `.mono_ns()` intrinsic, which cannot.

`.time_parts(...)` is UTC-only, so a timestamp shown to a reader needs the local
offset. `local_parts` applies it for you:

```fol
var fields: vec[int] = std::time::local_parts(std::time::now_ms() / 1000);
```

`local_offset` takes the instant because the offset is not a constant — the same
zone is `3600` in January and `7200` in July, and the daylight-saving rule in
force at that moment decides which.

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
