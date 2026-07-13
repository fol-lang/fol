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
- `ptr[shared, T]` values do not cross the boundary because their `Rc` backing
  is not thread-safe

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

Current examples:

- `examples/proc_select_m3`
- `examples/proc_mutex_m3`
- `examples/proc_mutex_explicit_unlock_m3`
