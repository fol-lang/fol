# Eventuals

Eventuals are a `V3` processor feature and are `std`-only. They use operating-
system threads; FOL does not use Rust `async`/`await`, futures, Tokio,
continuations, or colored routines.

The chosen pipe surface is:

```fol
var pending = calculate() | async;
var value = pending | await;
```

`| async` starts the call on an OS thread and produces an internal eventual.
`| await` blocks the current OS thread until that computation finishes. The
eventual type is not nameable in `V3`; `evt[T]` is only a possible later design
slot.

Error behavior stays identical to the synchronous call. An infallible call
awaits to `T`. A routine declared as `T / E` remains recoverable after await and
must be handled with the existing `check(...)` or `||` surfaces. Async and await
do not introduce a second error channel.

Program exit joins outstanding async work just as it joins bare `[>]` tasks.
Cancellation, worker pools, and runtime scheduling controls are not part of
`V3`.

The eventual slice is implemented end to end. See
`examples/proc_async_await_m4` and `examples/proc_await_error_m4`; attempts to
spell `evt[T]` are covered by `examples/fail_proc_evt_named_m4`.
