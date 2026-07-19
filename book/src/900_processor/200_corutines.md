# Tasks, Channels, and Mutexes

The processor surface is a `V3` systems feature. Every processor construct is
`std`-only: a package must declare the bundled internal `standard` dependency.
Choosing `core` or `memo` alone does not provide threads or hosted services.
The artifact itself uses `fol_model = "memo"`; there is no `std` model.
This dependency gates processor APIs, not process execution: std-free `core`
and `memo` artifacts may still run on a compatible host.

## Spawn

`[>]call()` starts one operating-system thread and lets the current routine
continue. FOL has no worker pool, scheduler settings, Rust async runtime, or
Tokio dependency. Every spawned task is registered with the process and the
program joins all outstanding tasks before its entry point exits.

```fol
use std: pkg = {"std"};

fun[] worker(): int = {
    return std::io::echo_int(17);
};

fun[] main(): int = {
    [>]worker();
    return 0;
};
```

A bare spawn is always fire-and-forget: `[>]call()` creates no eventual and
cannot be awaited. It must therefore call an infallible routine. To run a
recoverable routine asynchronously, omit `[>]`, write `call() | async`, then
consume the eventual with `| await` and handle the result with `check(...)` or
`||`.
The call form must resolve directly to a named routine declaration. Both an
unqualified call such as `[>]worker()` and a qualified call such as
`[>]workers::worker()` are supported. Stored routine values, stored anonymous
routines, and routine parameters remain indirect calls and are not spawn
targets in `V3`. The explicit zero-parameter anonymous spawn form remains
available for explicit channel sender-endpoint captures; it is not a general
closure-capture surface. Receiver-method call syntax is not a named
spawn-target form; use a free routine name or qualified path instead.

The spawn boundary follows the `V3` memory rules:

- clone-safe values clone into the task
- thread-safe move-only values, including `@` ownership and unique pointers,
  move into the task and leave the sender moved-out
- borrowed values do not cross the thread boundary
- `ptr[shared, T]` values do not cross the boundary because their `Rc` backing
  is not thread-safe
- unresolved generic parameters do not cross until FOL has a thread-safety and
  lifetime contract for generics; concrete thread-safe instantiations can cross
- omitted defaults are checked as task arguments under the same rules as
  explicit arguments

Cross-thread shared mutation belongs to `mux[T]` parameters, not `Rc`.

The exact positive and negative spawn examples are maintained in the
[canonical shipped processor inventory](./_index.md#shipped-example-inventory).

## Channels

Channels are an implemented processor surface. The contract is an unbounded
MPSC `chn[T]` backed by `std::sync::mpsc`: `c[tx]` sends without blocking,
`c[rx]` performs a blocking pull that yields `opt[T]` — its present branch owns
a fresh payload and `nil` means every sender has closed — and receiver iteration
runs until all sender handles are dropped. Unwrap the result with `c[rx][]` or
bind it and inspect it with `when ... on ... *`. The old sequence-index spelling
`c[rx][i]` is not part of the contract.

A send `value | c[tx]` produces a must-handle `err[T]`: `nil` means the payload
was delivered, and the present branch owns the unsent payload when every
receiver has already closed. Bind it (`var sent: err[int] = value | c[tx]`),
inspect it with `when ... on ... *`, or propagate it; a bare send that discards
the result — and with it the unsent payload — is rejected.

A transmitter is a first-class value. `c[tx]` has type `chn[tx, T]`, a
clone-capable sender endpoint that may be bound, passed to another routine, or
returned. Sending works through any sender value — `value | sender` — not only a
direct `c[tx]` access, so a producer can accept a `chn[tx, T]` parameter and emit
into it. The receiver endpoint is also a first-class `chn[rx, T]` value, but
unlike the cloneable sender it is unique and move-only: it may be bound, passed,
or returned, yet never duplicated, so a channel keeps exactly one receiver.

`T` may be a thread-safe move-only value. Sending consumes that value, and a
blocking receive, receiver iteration, or selected arm transfers the payload to
its destination without cloning it; the blocking receive delivers it inside the
`opt[T]` shell. Unique pointers are supported this way; values containing
`ptr[shared, T]` remain barred from OS-thread boundaries.

The first `c[rx]` acquisition relinquishes that channel binding's local
transmitter capability. Clone or capture every needed `c[tx]` handle before
receiving. Sender handles acquired earlier remain valid and the channel closes
when the last of those handles is dropped; trying to acquire `c[tx]` after the
receiver is active is an ownership error, not a runtime panic. This explicit
endpoint lifecycle lets pull loops and `select` observe closure without keeping
an invisible transmitter alive.

Channel endpoint acquisition is not allowed inside `dfr` or `edf`, including
endpoint captures on anonymous spawned routines in a deferred body. Acquire all
sender handles before entering the deferred block, and keep receiver operations
in ordinary control flow; delayed endpoint acquisition cannot be ordered safely
against the first receiver acquisition.

V3 endpoint access is restricted to a direct local, parameter, or capture
binding owned by the current routine. Projected channel fields, channel
container elements, and implicit references to an outer routine's channel are
not part of the shipped lifecycle model; keep the channel in a direct binding
before using `[tx]`, `[rx]`, iteration, or `select`. Top-level/global channel
bindings are also rejected; a channel belongs to the routine that owns its
receiver.

Full `chn[T]` values also cannot be embedded in records, entries, containers,
or ownership/error/optional wrappers in V3. This keeps the single receiver and
its endpoint lifecycle attached to one direct routine-local binding. Sender
handles that were acquired earlier remain independently cloneable.

Anonymous routines cannot declare `chn[T]` parameters in V3 because they do not
participate in named-routine sender/receiver effect refinement. Use a named
routine, or capture an already-existing sender explicitly with `c[tx]`.

Spawned anonymous routines can clone a transmitter explicitly with
`[>]fun()[c[tx]] = { ... }`. Receiver endpoints are not cloned: MPSC retains a
single consuming side.

The exact positive, ownership-boundary, lifecycle, and tier-failure channel
examples are maintained in the
[canonical shipped processor inventory](./_index.md#shipped-example-inventory).

## Select

The chosen multiplexing form is a multi-arm statement:

```fol
select {
    when first as value { consume(value); }
    when second as value { consume(value); }
};
```

The old single-channel `select(channel as name) { ... }` form is not retained.
The runtime polls arms in source order with `try_recv()`. Closed arms are
skipped and a blocking select completes, continuing after the statement, when
all arms are closed. An optional `*` arm runs immediately when no receiver is
ready. Simultaneously-ready arms therefore have source-order bias in V3; no
fairness guarantee is promised.

A blocking select without `*` must contain at least one channel arm.
`select {}` is invalid and is rejected during typecheck rather than being left
for lowering. A default-only select is non-blocking because its `*` body runs
immediately.

## Mutex parameters

The mutex is the first-class managed type `mux[T]`. The direct handle surface
locks and unlocks through the handle itself:

```fol
fun[] update(value: mux[Counter]): int = {
    value.lock();
    value.total = value.total + 1;
    var result: int = value.total;
    value.unlock();
    return result;
};
```

`mux[T]` lowers to `Arc<Mutex<T>>`. A routine acquires the guard with `.lock()`;
guarded fields are accessible only while that guard is active. The guard is
released automatically at the end of the lexical scope that acquired it, or
early with `.unlock()` in that same scope. Locking the same parameter twice is
rejected, as is unlocking without a guard acquired in the current lexical
scope. The historical `((name))` parameter spelling is not retained.

Mutex field access and `.lock()`/`.unlock()` are not allowed inside `dfr` or
`edf` bodies in `V3`. Guard transitions are tracked for immediate lexical
execution and are not replayed as delayed effects at scope exit. Forwarding a
mutex handle from a deferred body to another `mux[T]` routine is rejected for
the same reason.

The guarded `T` cannot be copied, returned, embedded, or passed to an ordinary
`T` parameter as a whole value. Passing the mutex handle directly to another
`mux[T]` parameter is allowed, including through spawn; data access still
requires that receiving routine to acquire its own guard.

For a synchronous call, the caller must unlock a handle before forwarding it
to another `mux[T]` parameter; otherwise the callee could block trying to acquire
the caller's guard. One call also cannot pass the same handle to two `mux[T]`
parameters because those aliases can self-deadlock. Spawn and async calls are
task boundaries and may receive the cloned handle while the caller still holds
its guard; the new task waits until the caller releases it.

### Guard values

Locking a borrowed handle produces a named, lifetime-scoped mutable guard
bound with `var[mut, bor]`. Field access goes through the guard while the lock
is held:

```fol
fun[] bump(state: mux[Counter]): int = {
    var[mut, bor] guard: Counter = ([bor]state).lock();
    guard.value = guard.value + 1;
    var snapshot: int = guard.value;
    [end]guard;
    return snapshot;
};
```

The guard releases at its last use (the same non-lexical rule as ordinary
loans), or early and explicitly with `[end]guard`. A guard value cannot be
moved, copied, or cloned, and it cannot be held across a spawn, an await, or a
blocking receive — the checker rejects those boundaries rather than attempting
static deadlock analysis. End the guard first, then cross the boundary
(`examples/proc_mutex_guard_end_m3` shows the early-end pattern;
`examples/fail_proc_mutex_guard_await_m3` and
`examples/fail_proc_mutex_guard_move_m3` pin the rejections).

The `mux[T]` calling convention is currently available only on named routines
that are called directly, through either unqualified or qualified names. A
named routine with `mux[T]` parameters cannot be stored or passed as a
first-class routine value, and anonymous routines cannot declare `mux[T]`
parameters. Those routine-value forms do not yet retain the mutex ABI metadata
needed to preserve `Arc<Mutex<T>>`; use a named direct call instead.

Despite this chapter's historical filename, the shipped surface is OS-thread
tasks, channels, select, and mutexes—not resumable coroutines or generators.
Generator `yield` is parser-recognized but remains rejected by typecheck and
lowering in `V3`.

The exact positive, removed-syntax, deferred-effect, and tier-failure examples
for `select` and mutexes are maintained in the
[canonical shipped processor inventory](./_index.md#shipped-example-inventory).
