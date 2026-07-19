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

`| async` starts the call on an OS thread and produces a one-shot eventual.
`| await` blocks the current OS thread until that computation finishes. A local
binding may name the eventual with its lexical lifetime elided (V3_MEM §8.1):
`evt[T]` for an infallible call and `evt[T / E]` for a recoverable one, e.g.
`var work: evt[int] = compute(7) | async`. The public lifetime-carrying
`evt[L, T]` spelling is also a namable type, naming the region the eventual
belongs to.

The call to the left of `| async` must resolve directly to a named routine
declaration. Both `calculate()` and a qualified call such as
`workers::calculate()` are supported; qualification selects a declaration and
does not introduce runtime dispatch. Stored routine values, stored anonymous
routines, and routine parameters are indirect calls and are not async task
targets in `V3`. Receiver-method call syntax is also excluded from the task
target surface; call a free named routine directly instead.

Nested routine bodies cannot implicitly capture outer locals. Pass ordinary
outer values through declared parameters instead of relying on a hidden closure
environment. An outer eventual cannot use that workaround because generic
crossings are forbidden; await and handle it in the outer routine.

Eventual bindings are move-only. A plain binding or assignment transfers the
eventual and makes the source binding unavailable. An eventual may be consumed
at most once: `| await` consumes the current binding, so a second await is a
use-after-consume error. An infallible eventual need not be awaited and is joined
at process exit; a recoverable eventual has the stronger must-handle rule below.
`V3` does not embed eventuals in composite values or pass them through generic
parameters, because generic bodies do not yet carry a move-only contract. Await
the value before crossing either boundary.

Arguments sent into `| async` obey the same thread boundary as `[>]`: borrowed
values, `ptr[shared, T]`, and unresolved generic parameters cannot cross it.
Omitted defaults are arguments too and are checked against that boundary before
the task starts. A generic call remains allowed when inference produces only
concrete thread-safe parameter types.

Thread-safe move-only results can cross back through an eventual. The producer
transfers the result into the eventual and `| await` transfers it to the
awaiting routine; neither step clones it. Unique pointers are one shipped
example of this rule. A result containing `ptr[shared, T]` is still rejected
because its `Rc` representation cannot cross the OS-thread boundary.

Error behavior stays identical to the synchronous call. An infallible call
awaits to `T`. A routine declared as `T / E` remains recoverable after await and
must be handled with the existing `check(...)` or `||` surfaces. Async and await
do not introduce a second error channel. A recoverable eventual is a must-handle
value: its final owner must await it and immediately handle the result. Moving it
to another binding moves that obligation. Lexical fallthrough and any `break`,
`return`, or `report` that exits a scope with a live obligation are rejected. At
a branch join, all continuing branches must leave compatible state: they may
all preserve the same owner for handling after the join, transfer consistently,
or discharge the obligation. Consuming or transferring on only some paths is
rejected. Dropping it as a statement, overwriting it while live, or discarding
the awaited result is also rejected.

`| await` is not allowed inside `edf`, and an existing eventual binding cannot
be accessed there. Error-only deferred cleanup does not run on normal exits, so
it cannot be used to discharge eventual ownership. This restriction remains in
force through nested blocks, including a `dfr` declared inside the `edf`. Await
or transfer the value in ordinary control flow instead.

Program exit joins outstanding async work just as it joins bare `[>]` tasks.
This includes an **infallible** eventual that was created and then never
awaited: it is not detached merely because no source-level binding consumes it.
Joining is not error handling, so a recoverable eventual cannot use that path to
discard its error. Cancellation, worker pools, and runtime scheduling controls
are not part of `V3`.

An eventual is a one-shot task result, not a resumable generator or coroutine.
`V3` does not execute generator `yield`; that statement remains recognized by
the parser but is rejected by semantic analysis and lowering. `async` also does
not color the called routine or change its declaration.

The eventual slice is implemented end to end. Its exact positive (including the
namable `evt[T]` local), tier-failure, and must-handle failure example set is
maintained in the
[canonical shipped processor inventory](./_index.md#shipped-example-inventory).
