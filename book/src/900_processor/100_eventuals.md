# Eventuals

Eventuals are a `V3` processor feature and are `std`-only. They use operating-
system threads; FOL does not use Rust `async`/`await`, futures, Tokio,
continuations, or colored routines.

The package therefore selects `fol_model = "memo"` for the artifact and
declares the bundled internal `standard` dependency. That declaration enables
the processor API; it is not what makes the executable runnable.

The chosen pipe surface is:

```fol
var pending = calculate() | async;
var value = pending | await;
```

`| async` starts the call on an OS thread and produces an internal eventual.
`| await` blocks the current OS thread until that computation finishes. The
eventual type is not nameable in `V3`; `evt[T]` is only a possible later design
slot.

The call to the left of `| async` must resolve directly to a named routine
declaration. Stored routine values and routine parameters are not async task
targets in `V3`.

Eventual bindings are move-only. A plain binding or assignment transfers the
eventual and makes the source binding unavailable; `| await` consumes the final
binding exactly once. `V3` does not embed eventuals in composite values or pass
them through generic parameters, because generic bodies do not yet carry a
move-only contract. Await the value before crossing either boundary.

Arguments sent into `| async` obey the same thread boundary as `[>]`: borrowed
values, `ptr[shared, T]`, and unresolved generic parameters cannot cross it.
Omitted defaults are arguments too and are checked against that boundary before
the task starts. A generic call remains allowed when inference produces only
concrete thread-safe parameter types.

Error behavior stays identical to the synchronous call. An infallible call
awaits to `T`. A routine declared as `T / E` remains recoverable after await and
must be handled with the existing `check(...)` or `||` surfaces. Async and await
do not introduce a second error channel.

Program exit joins outstanding async work just as it joins bare `[>]` tasks.
Cancellation, worker pools, and runtime scheduling controls are not part of
`V3`.

The eventual slice is implemented end to end. Its exact positive, tier-failure,
and unnameable-`evt[T]` example set is maintained in the
[canonical shipped processor inventory](./_index.md#shipped-example-inventory).
