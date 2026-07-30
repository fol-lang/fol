# V3 Processor History and Integration Index

> **Status: the integrated processor contract is implemented.** This file
> preserves the P1-P4/W-Z processor implementation record and points to the
> integrated replacement contract in `plan/V3_MEM.md`, which is implemented
> end to end. It is not a second normative implementation plan.

The normative ownership and processor design is
[`plan/V3_MEM.md`](./V3_MEM.md), especially:

- [Ownership Operations](./V3_MEM.md#2-ownership-operations)
- [Finalization and Delayed Cleanup](./V3_MEM.md#6-finalization-and-delayed-cleanup)
- [Processor Ownership Integration](./V3_MEM.md#8-processor-ownership-integration)
- [Compiler Architecture](./V3_MEM.md#9-compiler-architecture)
- [Vertical Delivery Plan](./V3_MEM.md#10-vertical-delivery-plan)
- [Tests and Completion Gates](./V3_MEM.md#11-tests-and-completion-gates)

When this historical record differs from `V3_MEM.md`, `V3_MEM.md` wins. The
root package remains version `0.2.0`.


# 1. Why This File Changed

The first processor pillar was implemented as a layer on top of the first
memory pillar. It reused that pillar's implicit type-directed move/clone rule
and treated task, channel, mutex, and eventual ownership as separate special
cases. The reopened V3 design instead requires them to use the same explicit
operations, places, loans, lifetimes, capabilities, cleanup plans, and CFG
obligations as ordinary FOL values.

Keeping the old 800-line checked-off plan as present-tense language policy
would create two contradictory specifications. This file therefore retains:

- what P1-P4 actually delivered;
- the backend and runtime provenance of that subset;
- links to its canonical checked-in inventories; and
- the exact migration from the shipped subset to the reopened contract.

All new implementation staging and completion gates live only in
`V3_MEM.md`.


# 2. Historical Shipped Processor Subset

This section is historical. It describes the subset before V3 deepening, not
the target contract being implemented now.

| Original milestone | Workstream | Shipped subset |
| --- | --- | --- |
| P1 | W | `[>]call` spawned a real OS thread and registered it for process-exit joining |
| P2 | X | `chn[T]`, `channel[tx]`/`channel[rx]`, pipe send, blocking pull, and channel iteration |
| P3 | Y | multi-arm `select` and `name[mux]: T` mutex parameters |
| P4 | Z | `call() \| async`, internal eventual values, and `eventual \| await` |

The delivery order was W -> X -> Y -> Z. Each milestone was delivered
vertically across parser, resolver, typecheck, lowering, backend, frontend
routing, diagnostics, formatter/tool commands, LSP, tree-sitter, examples,
tests, docs, and book chapters.

## 2.1 Runtime provenance

The shipped processor subset is bundled-`std`-only. Plain `core` and `memo`
source reject processor APIs unless the build adds the bundled standard
dependency. That is a source-language capability gate, not a claim that a host
frontend cannot launch an ordinary artifact.

The backend uses hosted Rust standard-library substrate:

- `std::thread` for thread-per-spawn execution;
- `std::sync::mpsc` for channels;
- `std::sync::Mutex` for guards; and
- `std::sync::Arc` for shared mutex handles.

It did not add Rust futures, Tokio, a FOL async runtime, a worker pool, or
scheduler controls. That provenance remains valid for the reopened design even
though its source semantics change.

## 2.2 P1/W: spawn

The shipped `[>]call` form:

- accepted direct named routine targets;
- used the old static clone-safe/move-only transfer classification;
- rejected non-thread-safe shared ownership crossing the boundary;
- rejected recoverable fire-and-forget callees; and
- joined outstanding task handles at process exit.

That was genuine OS-thread execution, not parser-only syntax. Its limitations
were process-global lifetime, implicit source operations, no borrowed scoped
tasks, and no general explicit capture model.

## 2.3 P2/X: channels

The shipped channel was unbounded MPSC:

- sender endpoints were cloneable;
- the receiver endpoint was unique;
- the channel closed after the final sender was dropped;
- iteration stopped on closure; and
- payloads/results could carry thread-safe move-only values.

`channel[rx][index]` was rejected; receive was a blocking pull or iteration.
Anonymous spawn capture was narrowly supported for sender endpoints.

The shipped send path used the old implicit transfer rule and did not expose
the unsent payload as the target contract now requires.

## 2.4 P3/Y: select and mutex parameters

The shipped multi-arm form was:

```fol
select {
    when first as value { consume(value); }
    when second as value { consume(value); }
    * { handle_not_ready(); }
};
```

Its historical behavior was:

- a blocking select required at least one channel arm;
- closed arms were skipped;
- selection exited after every arm closed;
- an optional `*` arm ran immediately when no input was ready; and
- simultaneously ready arms were checked in source order.

The old single-channel `select(channel as value)` grammar was removed.

Mutex sharing used `name[mux]: T` parameter metadata. `.lock()` activated
guarded field access and `.unlock()` or scope exit released it. The earlier
`((name))` parameter form was removed. This was not yet a first-class owned
mutex type and could not participate uniformly in ordinary generics, storage,
or lifetime relationships.

## 2.5 P4/Z: eventuals

The shipped form was:

```fol
var work = compute() | async;
var value = work | await;
```

An eventual was internal and could not appear as a user-written type. Await
preserved the synchronous call's recoverable contract, and recoverable
eventuals were must-handle values. Infallible unawaited eventuals were joined
at process exit.

The reopened design keeps one-shot await and error transparency but replaces
the internal/process-global model with public, lifetime-carrying scoped
eventuals.


# 3. Canonical Historical Evidence

Do not duplicate the complete example matrix here. The canonical checked-in
processor inventory is maintained in:

- [`book/src/900_processor/_index.md`](../book/src/900_processor/_index.md#shipped-example-inventory)
- `test/v3_example_inventory.rs`
- the `examples/proc_*` and `examples/fail_proc_*` package directories

The primary historical book chapters are:

- [`book/src/900_processor/100_eventuals.md`](../book/src/900_processor/100_eventuals.md)
- [`book/src/900_processor/200_corutines.md`](../book/src/900_processor/200_corutines.md)

Those artifacts are regression evidence, not permission to preserve obsolete
syntax. Positive packages migrate in the vertical slice that changes their
contract. Focused failure packages retain removed syntax when needed to prove
the hard break. The inventory and book must change in the same revision as the
compiler surface they describe.

Historical dead-form coverage includes:

- single-channel `select(channel as value)`;
- `((name))` mutex parameters;
- indexed channel receive `channel[rx][index]`;
- processor use in an unsupported capability tier;
- unsafe/non-thread-safe ownership crossing spawn; and
- unhandled recoverable eventuals.


# 4. Migration to the Normative Integrated Model

This table is the boundary between historical behavior and the new contract.
The right column is only a summary; normative detail remains in `V3_MEM.md`.

| Historical subset | Reopened V3 contract |
| --- | --- |
| implicit clone-safe/move-only task transfer | caller writes `[mov]`, `[cpy]`, `[cln]`, `[bor]`, or `[mut, bor]` |
| `[>]call` joined at process exit | `[spn]call` is lexically scoped; `[>]` is its shorthand |
| no detached distinction | `[spn, det]call` is explicit, owned-`send` only, infallible, and not exit-joined |
| no borrowed task capture | scoped tasks may capture loans under `send`/`share` checks |
| restricted/implicit captures | anonymous tasks and delayed blocks list `name[operation]` captures |
| internal eventual | public `evt[L, T]` and `evt[L, T / E]` |
| process-global join | lexical join on every scope exit before deferred/final cleanup |
| implicit channel payload transfer | `[mov]value \| sender`, `[cpy]value \| sender`, or `[cln]value \| sender` |
| send failure hidden by backend path | must-handle `err[T]` returns the unsent payload |
| receive produced a bare payload | blocking receive produces `opt[T]` |
| endpoints only as `channel[tx/rx]` access | public `chn[tx, T]` and `chn[rx, T]` endpoint types |
| `name[mux]: T` parameter metadata | first-class `mux[T]` managed value |
| lexical `.lock()`/`.unlock()` state | lifetime-bound mutable guard, ended by NLL or `[end]guard` |
| special processor ownership checks | the shared place/effect CFG and capability solver |

The following processor invariants remain:

- the entire processor API is bundled-`std`-only;
- execution uses real OS threads without a Rust async runtime;
- channels are unbounded MPSC and close after the last sender is dropped;
- select skips closed arms and preserves its documented default/source-order
  behavior; and
- generators/coroutines and executable `yield` remain outside V3.


# 5. New Processor Completion Gate

The processor portion of reopened V3 is complete only when the integrated
completion rule in `V3_MEM.md` passes. In particular:

1. tasks, eventuals, channels, endpoints, send results, mutexes, and guards use
   the same typed place/effect IR as ordinary values;
2. scoped task loans and handles cannot outlive their parent lifetime;
3. every language-controlled scope exit joins all scoped tasks before delayed
   blocks and finalization;
4. recoverable eventuals and failed-send payloads cannot be discarded
   implicitly;
5. detached tasks accept no borrows or recoverable callees and are not joined
   at normal process exit;
6. mutex guards cannot escape or cross spawn, await, blocking receive, or
   blocking select;
7. `core` and plain `memo` tier failures remain enforced end to end;
8. direct compiler, frontend, LSP, tree-sitter, examples, book, and inventory
   surfaces agree; and
9. checker-approved processor programs do not fail emitted Rust ownership or
   borrow checking.

Validation follows the repository Makefile, culminating in:

```text
make test
make tree-test
make docs TYPE=mdbook
make verify
```


# 6. Historical and Current Non-Goals

The shipped subset did not provide, and reopened V3 does not add:

- Rust async/futures/Tokio or colored functions;
- a worker pool, scheduler knobs, or task cancellation;
- bounded, MPMC, back-pressure, cross-process, or network channels;
- resumable generators/coroutines or executable `yield`;
- implicit runtime dispatch; or
- unsafe cross-thread sharing that bypasses `send`/`share` and the ownership
  checker.

The following are no longer valid non-goals because the reopened contract
deliberately adds them: public eventual types, scoped borrowed tasks,
first-class mutexes, typed endpoint values, explicit failed-send recovery, and
the integration of task cleanup with finalization.


# 7. Historical Record Rule

Repository history retains the original detailed W/X/Y/Z workstream plan and
its checked boxes. This condensed file preserves its meaning without presenting
obsolete behavior as the language target.

Do not reintroduce a second processor roadmap here. Future processor syntax,
semantics, staging, diagnostics, or acceptance changes must update the
normative sections of `V3_MEM.md` first, then update this file only when the
historical boundary or navigation links change.
