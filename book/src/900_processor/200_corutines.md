# Tasks, Channels, and Mutexes

The processor surface is a `V3` systems feature. Every processor construct is
`std`-only: a package must declare the bundled internal `standard` dependency.
Choosing `core` or `memo` alone does not provide threads or hosted services.

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

A bare spawn must be infallible. Spawning a routine declared with `/ ErrorType`
without awaiting it is rejected because the error would otherwise disappear.

The spawn boundary follows the `V3` memory rules:

- stack values clone into the task
- `@` values move into the task, leaving the sender moved-out
- borrowed values do not cross the thread boundary
- `ptr[shared, T]` values do not cross the boundary because their `Rc` backing
  is not thread-safe
- unresolved generic parameters do not cross until FOL has a thread-safety and
  lifetime contract for generics; concrete thread-safe instantiations can cross
- omitted defaults are checked as task arguments under the same rules as
  explicit arguments

Cross-thread shared mutation belongs to `[mux]` parameters, not `Rc`.

Current implementation examples:

- `examples/proc_spawn_m1`
- `examples/proc_spawn_move_heap_m1`
- `examples/fail_proc_spawn_in_memo_m1`
- `examples/fail_proc_spawn_rc_cross_m1`
- `examples/fail_proc_spawn_recoverable_m1`
- `examples/fail_proc_spawn_heap_use_after_move_m1`

## Channels

Channels are an implemented processor surface. The contract is an unbounded
MPSC `chn[T]` backed by `std::sync::mpsc`: `c[tx]` sends without blocking,
`c[rx]` performs a blocking pull, and receiver iteration runs until all sender
handles are dropped. The old sequence-index spelling `c[rx][i]` is not part of
the contract.

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

Current examples:

- `examples/proc_channel_m2`
- `examples/proc_channel_pull_m2`
- `examples/proc_channel_capture_m2`
- `examples/fail_proc_channel_index_m2`
- `examples/fail_proc_channel_in_core_m2`

## Select

The chosen multiplexing form is a multi-arm statement:

```fol
select {
    when first[rx] as value { consume(value); }
    when second[rx] as value { consume(value); }
}
```

The old single-channel `select(channel as name) { ... }` form is not retained.
The runtime polls arms in source order with `try_recv()`. Closed arms are
skipped and a blocking select completes when all arms are closed. An optional
`*` arm runs immediately when no receiver is ready. Simultaneously-ready arms
therefore have source-order bias in V3; no fairness guarantee is promised.

## Mutex parameters

The chosen mutex surface uses the ordinary parameter-option seam:

```fol
fun[] update(value[mux]: Counter): int = {
    value.lock();
    value.total = value.total + 1;
    var result: int = value.total;
    value.unlock();
    return result;
};
```

`[mux]` lowers to `Arc<Mutex<T>>`. A routine acquires the guard with `.lock()`;
guarded fields are accessible only while that guard is active. The guard is
released automatically at the end of the lexical scope that acquired it, or
early with `.unlock()` in that same scope. The historical `((name))` parameter
spelling is not retained.

The guarded `T` cannot be copied, returned, embedded, or passed to an ordinary
`T` parameter as a whole value. Passing the mutex handle directly to another
`[mux]` parameter is allowed, including through spawn; data access still
requires that receiving routine to acquire its own guard.

Current examples:

- `examples/proc_select_m3`
- `examples/proc_mutex_m3`
- `examples/proc_mutex_explicit_unlock_m3`
