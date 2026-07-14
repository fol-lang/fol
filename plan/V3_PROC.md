# V3 Processor Pillar Plan

> **Status: complete.** This file is retained as the implementation record for
> the shipped V3 processor pillar. The current user-facing contract lives in the
> linked book chapters and `plan/VERSIONS.md`. The exact checked-in processor
> examples are maintained in the
> [canonical shipped inventory](../book/src/900_processor/_index.md#shipped-example-inventory).
> Present-tense planning language below describes the original staging work,
> not unfinished current behavior.

V3 is the systems-semantics release, split into two pillars. The **memory
pillar** (`plan/V3_MEM.md`) lands first. This plan covers the **processor
pillar**: OS-thread task spawning, channels, mutexes, and eventuals.

The theme is:

- the **entire** processor surface is **bundled-`std`-only**; plain `core` and
  `memo` reject it with tier diagnostics that follow the existing `.echo(...)`
  gate
- that bundled-`std`-only rule gates processor APIs, not execution itself;
  ordinary `core` and `memo` artifacts may run and test without bundled `std`
- `std` is not a public `fol_model`; a `memo` build reaches the hosted tier only
  through
  `build.add_dep({ alias = "std", source = "internal", target = "standard" })`
- `graph.add_run(...)` / `graph.add_test(...)` do not imply that dependency;
  current execution remains host-only, so cross-target artifacts need an
  external runner outside those commands
- concurrency is real OS threads through the Rust standard library — **no** Rust
  async/futures/tokio, so FOL never grows colored functions or a runtime
  dependency
- the spawn boundary reuses the memory pillar's static move/clone rule:
  clone-safe values clone, thread-safe move-only values transfer, and `Rc`
  never crosses
- direct named spawn and async targets may be unqualified or qualified; stored
  routine values, anonymous routine values stored in bindings, and routine
  parameters remain indirect targets and are rejected; receiver-method call
  syntax is not a task-target form in V3
- nested routine bodies do not implicitly capture outer locals; required values
  must cross through declared parameters; the shipped explicit capture form is
  limited to channel sender endpoints on anonymous spawns
- channels and eventuals transfer thread-safe move-only payloads/results rather
  than cloning them
- blocking `select` requires at least one channel arm, exits when every arm is
  closed, and checks simultaneously-ready arms in source order
- program exit **joins** all outstanding tasks, including infallible eventuals
  that were never awaited; recoverable eventuals remain must-handle values
- `| await` and access to an existing eventual binding are rejected inside
  error-only `edf` cleanup, including through a `dfr` nested inside that `edf`
- every feature change is mirrored through frontend capability routing,
  structured diagnostics and explanations, formatter/tool commands, the LSP,
  tree-sitter grammar/queries/corpus, examples, tests, docs, and the book in the
  **same** change set, never later

The book chapters this plan implements — and heavily rewrites — are:

- `book/src/900_processor/100_eventuals.md`
- `book/src/900_processor/200_corutines.md`

Every place this plan contradicts those chapters is enumerated in Workstream AA.


# 1. Guardrails

Keep, permanently:

- bundled-`std`-only tiering for the whole processor surface; this is source
  API legality, not permission to launch a host-compatible artifact
- OS-thread execution via `std::thread`, `std::sync::mpsc`, `std::sync::Mutex`,
  `std::sync::Arc`
- the memory pillar's static move/clone rule at every task-transfer boundary and
  explicit channel endpoint-capture boundary
- join-all-tasks at process exit
- direct unqualified and qualified named routine targets for spawn/async, while
  indirect routine-value targets remain rejected
- error handling that is **identical** to the synchronous call site (no new error
  surface at `async`/`await`)
- monomorphized, dispatch-free lowering

Never add, under any workstream in this plan:

- Rust `async`/`await`, futures, or tokio (no colored functions, no async
  runtime dependency)
- a worker pool or scheduler knobs (thread-per-spawn only for V3)
- bounded/buffered channels, MPMC, or channel back-pressure
- cancellation of running tasks
- a user-nameable eventual type (`evt[T]` stays internal in V3)
- cross-process / network channels
- sharing `Rc` (or any non-thread-safe value) across a spawn boundary
- implicit runtime dispatch of any kind
- generator/coroutine execution or an executable `yield` statement; `yield`
  remains parser-recognized but semantically rejected in V3

If a workstream drifts toward these, it stops.

Dependency on the memory pillar (`plan/V3_MEM.md`):

- this plan starts only after the memory pillar completes (Q -> R -> S -> T)
- it consumes the memory pillar's **move-at-boundary** rule (clone-safe values
  clone, thread-safe move-only values transfer, and `Rc` never crosses) and the
  **`name[options]: type`** parameter-option grammar seam (Q5), which the
  `[mux]` parameter reuses
- keyword hygiene from prep (Q1/Q2) is assumed: `dfr` spelling, no `go`


# 2. Pre-Implementation Truth Snapshot (Historical)

This was the verified baseline before the processor pillar landed. It is
preserved to explain the workstream decisions and must not be read as current
compiler behavior.

Parsed at that baseline, but was semantically rejected:

- `[>]expr` spawn parses to `AstNode::Spawn { task }`
  (`primary_expression_parsers.rs:167`, `ast/node.rs:202`). Typecheck rejects it
  at `exprs/mod.rs:234` ("spawn expressions are planned for a future release").
- `| async` / `| await` pipe stages parse to `AstNode::AsyncStage` /
  `AstNode::AwaitStage` (`pipe_expression_parsers.rs:125,130`,
  `ast/node.rs:196,199`). Typecheck rejects them at `exprs/mod.rs:225,229` and
  `exprs/operators.rs:41,51`.
- `chn[T]` type parses to `FolType::Channel { element_type }` — **exactly one**
  type argument (`special_type_parsers.rs:275`, `ast/types.rs:110`). Typecheck
  rejects channel types at `decls.rs:3289`.
- `c[tx]` / `c[rx]` endpoint access parses to `AstNode::ChannelAccess { channel,
  endpoint }` with `ChannelEndpoint::Tx`/`Rx`
  (`access_expression_parsers.rs`, `ast/node.rs:269`). Typecheck rejects endpoint
  access at `exprs/mod.rs:580`.
- `select(chan as c) { ... }` parses to `AstNode::Select { channel, binding,
  body }` (`statement_parsers.rs:4`) — a **single-channel header** form, **not**
  a multi-arm `when`-branch form. Typecheck rejects channel `when`/`on` branches
  at `exprs/controlflow.rs:54` and select/channel semantics at `exprs/mod.rs:615`.
- mutex parameters used the `((name))` **double-paren** form at that baseline,
  which set `Parameter.is_mutex = true`
  (`routine_header_parsers.rs:56`, `ast/types.rs:253`).
  Typecheck rejects mutex params at `decls.rs:3453`.

Did **not** parse at that baseline — real grammar work was required:

- `select { when c1 as x { ... } when c2 as y { ... } }` multi-arm select — the
  baseline `select(...)` was a single-channel header, so the multiplexing form
  is new grammar; the single-channel `select(...)` form is **dead** and is removed.
- `name[mux]: T` mutex parameter — parameters did not parse the option bracket
  at that baseline; this reuses the memory pillar's Q5 seam, and the `((name))`
  double-paren form is **dead** and is removed.
- `chn[T, N]` bounded channel — `chn[...]` accepts exactly one arg (future slot,
  not V3).

Tier gating already exists: `TypecheckCapabilityModel` (Core/Memo/Std) drives
`fol-typecheck/src/model.rs`, and `.echo(...)` is already `std`-gated the same
way the whole processor surface will be.

Diagnostics: the `O####` OWNERSHIP family added in the memory pillar (Q4) is
reused here for move-at-boundary and `Rc`-crossing-spawn violations; tier
violations use the existing `std`-gate diagnostic shape (TYPES family), like
`.echo`. No new family is introduced by this plan.


# 3. Exit Criteria

This plan is complete when all of the following are true:

1. P1 (W), P2 (X), P3 (Y), and P4 (Z) are each implemented end to end — parser,
   resolver, typecheck, lowering, runtime/backend, frontend routing,
   diagnostics/explanations, formatter/tool commands, LSP, tree-sitter
   grammar/queries/corpus, positive and `fail_*` inventories, docs, and the book
   — and each landed workspace-green.
2. The entire processor surface is `std`-only: `core` and `memo` builds reject
   every processor construct with a tier diagnostic, verified by `fail_*`
   examples.
3. No "planned for a future release" rejection remains in the compiler for a
   processor surface this plan has chosen to build; each such site is deleted.
4. The dead forms are gone from the grammar: the single-channel `select(...)`
   header and the `((name))` double-paren mutex parameter no longer parse.
5. Backend emission uses only `std::thread` / `std::sync::mpsc` /
   `std::sync::Mutex` / `std::sync::Arc` — no async runtime, no worker pool.
6. Process exit joins all outstanding tasks in every positive example.
7. The V3 processor chapters describe the resulting language exactly, with no
   leftover pre-rewrite wording (Workstream AA).


# 4. Workstream W: P1 — Spawn and the `std`-Only Tier Gate

Goal: `[>]expr` spawns a real OS thread, the tier gate for the whole processor
surface is established, and the spawn boundary reuses the memory pillar's static
move/clone rule.

## W1. The `std`-only tier gate

The whole processor surface is gated to `std`. Spawn is the first construct to
gate, so W establishes the gate that X, Y, and Z reuse.

Work:

- typecheck: in `fol-typecheck/src/model.rs`, reject `[>]`, `chn[T]`, endpoint
  access, `select`, `[mux]`, `| async`, and `| await` in `core`/`memo` with the
  same tier-diagnostic shape as `.echo(...)`; allow them in `std`
- keep one honest tier message per construct, citing the `std` requirement

## W2. Spawn execution

Backend model:

- thread-per-spawn via `std::thread::spawn` — **no** worker pool (a pool is an
  explicit later slot), **no** Rust async/tokio
- program exit **joins** all outstanding tasks (collect join handles, join before
  `main` returns)

Work:

- typecheck: delete the spawn rejection (`exprs/mod.rs:234`); accept a direct
  call to a named routine declaration through either an unqualified name or a
  qualified path, plus the explicit zero-parameter anonymous spawn form;
  stored routine values and routine parameters remain unsupported indirect
  task targets, and receiver-method call syntax remains excluded
- lowering/backend: emit `std::thread::spawn(move || { ... })`; register the join
  handle in a process-level set joined at exit

## W3. Argument transfer at the spawn boundary

Rule (reuses the memory pillar):

- **clone-safe** call arguments **clone** into the generated spawned closure
- **thread-safe move-only** values, including `@` ownership, unique pointers,
  and aggregates containing them, **move** into that closure; the caller
  loses ownership (use-after-move is an `O`-code error via the memory pillar's
  checker)
- **`ptr[shared, T]` (`Rc`)** crossing a spawn boundary is an **`O`-code / tier
  compile error** — `Rc` is not thread-safe; sharing across threads is done
  exclusively through `[mux]` parameters (Y)

Work:

- typecheck: at each `[>]` boundary, run the memory pillar's recursive
  move/clone rule over task-call arguments; emit the `Rc`-crossing `O`
  diagnostic; mark moved arguments moved-out in the caller
- lowering: emit `move ||` Rust closures; clone clone-safe arguments and
  transfer move-only arguments into that generated closure
- channel payloads and eventual results use the same ownership rule: a
  thread-safe move-only value crosses exactly once and is not cloned;
  `ptr[shared, T]` remains forbidden at the task boundary

## W4. Fire-and-forget requires an infallible callee

A bare `[>]call()` is fire-and-forget and creates no eventual, so it can never be
awaited. It must call an **infallible** routine. Using it with a recoverable
routine is a compile error because the error would silently vanish. The
awaitable alternative is `call() | async`, followed by `| await` and immediate
`check(...)` or `||` handling.

Work:

- typecheck: if `[>]expr`'s callee is recoverable (has an error type), reject
  with a precise diagnostic ("bare `[>]call()` cannot spawn a recoverable
  routine; remove `[>]`, use `call() | async`, then await and handle it")

This rule applies equally to unqualified and qualified named calls. Qualification
chooses a declaration; it does not turn the call into indirect dispatch.

Editor / tree-sitter for W:

- tree-sitter: `[>]` already parses — verify the `Spawn` node shape; add a
  highlight for the spawn sigil; validate zero ERROR nodes
- LSP hover on `[>]call()` shows "spawns a task (joined at process exit)"; hover
  on a moved `@` argument explains the transfer
- LSP diagnostics surface the tier gate, the `Rc`-crossing error, and the
  recoverable-fire-and-forget error at the spawn site

Original milestone seed examples (the complete current matrix is the
[canonical shipped inventory](../book/src/900_processor/_index.md#shipped-example-inventory)):

- positive: `examples/proc_spawn_m1` — spawn an infallible routine, join at exit
- positive: `examples/proc_spawn_move_heap_m1` — a spawn call transfers an `@`
  argument by move
- negative: `examples/fail_proc_spawn_in_memo_m1` — spawn in a `memo` build
  (tier)
- negative: `examples/fail_proc_spawn_rc_cross_m1` — pass an `Rc` argument across
  a spawn boundary (`O` code)
- negative: `examples/fail_proc_spawn_recoverable_m1` — fire-and-forget a
  recoverable routine

Docs for W: see Workstream AA (task/channel chapter: `[>]`, worker wording,
join-at-exit, task-boundary transfer rules).

Tracked slices:

- [x] W1. `std`-only tier gate for the whole processor surface, established at
  spawn.
- [x] W2. `[>]` thread-per-spawn backend + join-all-at-exit; delete the spawn
  rejection.
- [x] W3. Argument transfer rule at the spawn boundary (clone-safe values clone,
  thread-safe move-only values transfer, `Rc` rejected).
- [x] W4. Fire-and-forget requires an infallible callee.
- [x] W5. LSP + tree-sitter: spawn hover/diagnostics, sigil highlight.
- [x] W6. Positive and `fail_*` spawn examples build/run/reject as specified.


# 5. Workstream X: P2 — Channels

Goal: `chn[T]` MPSC channels with pipe-send and a blocking-pull receive
expression, plus channel iteration.

## X1. Channel semantics

Fixed rules:

- `chn[T]` is **MPSC**, **unbounded**; `tx` never blocks, `rx` blocks
- a channel **closes** when all `tx` handles are dropped
- the first `c[rx]` acquisition consumes the channel binding's local ability to
  create more `tx` handles; all needed senders must be cloned/captured first,
  and already-created handles remain valid until dropped
- sending a move-only `T` consumes the sender-side value, and receiving or
  iterating transfers that payload out of the channel without cloning it
- backend: `std::sync::mpsc`

Work:

- typecheck: delete the channel-type rejection (`decls.rs:3289`) and the endpoint
  rejection (`exprs/mod.rs:580`); type `c[tx]` as the transmitter and `c[rx]` as
  the receiver over `T`
- lowering/backend: emit `std::sync::mpsc::channel::<T>()`; map endpoint access to
  the sender/receiver halves; close on last-sender drop is the mpsc default

## X2. Send and receive surface

- **send** via pipe: `expr | c[tx]`
- **receive** is a **blocking pull expression**: `var x = c[rx]` — each
  evaluation pulls the next message (there is **no** `[rx][i]` indexing; the
  book's seq-index model is dead)
- **iteration**: `for msg in c[rx] { ... }` runs until the channel is closed

Work:

- parser: confirm `expr | c[tx]` parses through the pipe grammar with a
  `ChannelAccess` right-hand side; confirm `for msg in c[rx]` parses `c[rx]` as an
  iterable expression
- typecheck: `c[rx]` as a receive expression yields `T`; `for ... in c[rx]`
  iterates `T` until close; **remove** any `[rx][i]` indexing path — indexing a
  receiver is an error
- typecheck: reject any attempt to acquire `c[tx]` after `c[rx]` activated the
  receiver, including through aliases, captures, and sender-only wrapper calls
- V3 lifecycle boundary: endpoint bases (including bare `select` receivers)
  must be direct local, parameter, or capture bindings owned by the current
  routine; projected fields, container elements, and implicit outer-routine
  references are not shipped until lifecycle tracking can model those paths;
  top-level/global channel bindings are rejected so they cannot bypass routine
  receiver ownership through a local alias
- full `chn[T]` values cannot be embedded in records, entries, containers, or
  wrapper types; direct routine-local bindings/parameters own receivers while
  already-acquired sender handles remain cloneable
- anonymous routines cannot declare `chn[T]` parameters; use a named routine
  that participates in sender/receiver refinement or capture an existing
  `c[tx]` sender explicitly
- cross-feature boundary: reject channel endpoint acquisition inside `dfr` and
  `edf`, including endpoint captures on deferred anonymous tasks; acquire sender
  handles before deferral and perform receiver operations in ordinary control
  flow
- lowering/backend: `| c[tx]` emits `tx.send(expr)`; `var x = c[rx]` emits a
  blocking `rx.recv()`; `for msg in c[rx]` emits `for msg in rx { ... }`

## X3. Endpoint capture in spawned routines

Spawned anonymous routines capture endpoints via the existing capture syntax:
`[>]fun()[c[tx]] = { ... }`.

Work:

- typecheck: an endpoint captured into a spawn moves/clones per the sender/
  receiver rules (a `tx` clone is legal — mpsc `Sender` is `Clone`; the channel
  stays open while any clone lives); dropping the last captured `tx` closes the
  channel
- lowering/backend: emit `tx.clone()` into `move ||` closures as needed

Editor / tree-sitter for X:

- tree-sitter: `chn[T]` and `c[tx]`/`c[rx]` already parse — verify node shapes;
  add highlights for the `tx`/`rx` endpoint keywords; validate zero ERROR nodes
- LSP hover on `c[rx]` shows "blocking receive of `T`"; hover on `c[tx]` shows
  "non-blocking send"; hover on `for msg in c[rx]` shows "iterate until closed"
- LSP diagnostics reject `[rx][i]` indexing with a message pointing at the pull
  expression / iteration

Original milestone seed examples (the complete current matrix is the
[canonical shipped inventory](../book/src/900_processor/_index.md#shipped-example-inventory)):

- positive: `examples/proc_channel_m2` — spawn producers, send via `| c[tx]`,
  drain via `for msg in c[rx]`
- positive: `examples/proc_channel_pull_m2` — a single blocking `var x = c[rx]`
  pull
- positive: `examples/proc_channel_capture_m2` — `[>]fun()[c[tx]] = { ... }`
  endpoint capture
- negative: `examples/fail_proc_channel_index_m2` — `c[rx][0]` indexing rejected
- negative: `examples/fail_proc_channel_in_core_m2` — channel in a `core` build
  (tier)

Docs for X: see Workstream AA (channel section: MPSC/unbounded, pipe send,
pull/iterate, dead `[rx][i]`).

Tracked slices:

- [x] X1. `chn[T]` MPSC/unbounded semantics + endpoint typing; delete channel and
  endpoint rejections.
- [x] X2. `expr | c[tx]` send, `var x = c[rx]` blocking pull, `for msg in c[rx]`
  iteration; remove `[rx][i]` indexing.
- [x] X3. Endpoint capture in spawned routines with sender-clone / last-drop-
  closes semantics.
- [x] X4. LSP + tree-sitter: endpoint hover/diagnostics, index-rejection message.
- [x] X5. Positive and `fail_*` channel examples build/run/reject as specified.


# 6. Workstream Y: P3 — Select Multiplexing and Mutex Parameters

Goal: a real multi-arm `select` and mutex parameters via the `[mux]` option.

## Y1. Select multiplexing

Surface:

```fol
select {
    when c1 as x { ... }
    when c2 as y { ... }
}
```

Reuse the `when`-branch shapes already used elsewhere. The planning questions
were resolved by the shipped contract as follows:

- each arm waits on one receiver; the first ready arm runs
- a blocking form without `*` must contain at least one channel arm;
  `select {}` is rejected during typecheck rather than reaching lowering
- **closed-channel arm**: an arm whose channel is closed is skipped; when all
  arms are closed, `select` completes and execution continues after the
  statement
- **default arm**: an optional `*` arm runs immediately when no channel is
  ready, making that selection non-blocking
- **ordering/fairness**: ready arms are polled in source order; V3 deliberately
  promises no fairness beyond that deterministic bias

Work:

- parser: **replace** the single-channel `select(chan as c) { }` form
  (`statement_parsers.rs:4`) with the multi-arm `select { when ... }` form; the
  old form is deleted (legacy policy)
- AST: represent `select` as a set of `when <receiver> as <name>` arms plus an
  optional default arm
- typecheck: delete the channel-`when`/`on` rejection (`exprs/controlflow.rs:54`)
  and the select rejection (`exprs/mod.rs:615`); each arm binds its message; type
  each arm body against the bound message type; reject an empty blocking select
  before lowering
- lowering/backend: `std::sync::mpsc` has no native select; emit a poll loop over
  `try_recv()` across the arms (documented as the V3 strategy), respecting the
  closed-arm and default-arm rules

## Y2. Mutex parameters

Surface: `name[mux]: T` — a mutex parameter; backend `Arc<Mutex<T>>`.

Intrinsics: `.lock()` / `.unlock()`; auto-unlock at scope end.

Work:

- parser: attach `[mux]` semantics to the memory pillar's `name[options]:` seam
  (Q5); set `Parameter.is_mutex = true` from the explicit option; **delete** the
  `((name))` double-paren mutex form (`routine_header_parsers.rs:56`)
- typecheck: delete the mutex-param rejection (`decls.rs:3453`); a `[mux]`
  parameter is `Arc<Mutex<T>>`; `.lock()` yields a writable guard, `.unlock()`
  releases early only in the lexical scope that acquired it, and the guard
  auto-releases at that scope's end; double-locking the same parameter and
  unlocking without a current-scope guard are rejected
- whole-value use of a `[mux]` parameter is forbidden; only guarded field
  access and mux-to-mux handle passing are legal
- a synchronous call cannot forward a mutex handle while its guard is active,
  and one call cannot pass the same handle to two `[mux]` parameters because
  either form can self-deadlock; spawn/async task calls may receive a cloned
  handle while the caller holds its guard, and the task blocks until that guard
  is released
- mutex field access, lock/unlock, and handle forwarding are rejected inside
  `dfr`/`edf`; delayed guard effects are not modeled in V3
- lowering/backend: emit `Arc<Mutex<T>>` params; `.lock()` -> `.lock().unwrap()`
  guard bound to the scope; `.unlock()` -> drop the guard early; auto-unlock is
  the guard's scope-end drop
- this is the sanctioned way to share mutable state across spawns (W3's
  `Rc`-crossing ban points here); an `Arc<Mutex<T>>` crossing `[>]` is legal
- `[mux]` parameters are available only on named routines reached by direct
  calls, including qualified calls; anonymous `[mux]` parameters and stored
  first-class mutex routines are not supported

Editor / tree-sitter for Y:

- tree-sitter: add the multi-arm `select { when ... }` rule (removing the old
  single-channel header); add the `[mux]` parameter option; add highlights;
  validate zero ERROR nodes
- LSP hover on a `select` arm shows the bound message type; hover on a `[mux]`
  param shows "mutex-guarded shared `T` (auto-unlock at scope end)"
- LSP diagnostics reject the old `select(...)` and `((...))` forms as no longer
  valid syntax

Original milestone seed examples (the complete current matrix is the
[canonical shipped inventory](../book/src/900_processor/_index.md#shipped-example-inventory)):

- positive: `examples/proc_select_m3` — multiplex two channels
- positive: `examples/proc_mutex_m3` — spawn workers sharing a `[mux]` value,
  `.lock()`/auto-unlock
- positive: `examples/proc_mutex_explicit_unlock_m3` — early `.unlock()`
- negative: `examples/fail_proc_select_old_form_m3` — `select(c as x) { }` no
  longer parses
- negative: `examples/fail_proc_select_empty_m3` — an empty blocking
  `select {}` is rejected during typecheck
- negative: `examples/fail_proc_mutex_double_paren_m3` — `((x))` no longer parses
- negative: `examples/fail_proc_mutex_in_memo_m3` — `[mux]` in a `memo` build
  (tier)
- negative: `examples/fail_proc_mutex_deferred_m3` — deferred mutex field and
  guard effects remain outside the current lexical guard model

Docs for Y: see Workstream AA (mutex section: `[mux]` not `((x))`; select
section: `when` arms not `[rx][c]`).

Tracked slices:

- [x] Y1. Multi-arm `select { when ... }` (parser replace, AST, typecheck,
  poll-loop lowering) with honest closed-arm/default-arm/fairness notes; delete
  the old `select(...)` form and its rejections.
- [x] Y2. `[mux]` mutex parameters via the Q5 seam (`Arc<Mutex<T>>`,
  `.lock()`/`.unlock()`, auto-unlock); delete the `((x))` form and the mutex
  rejection.
- [x] Y3. LSP + tree-sitter: select arms, `[mux]` param, dead-form rejection.
- [x] Y4. Positive and `fail_*` select/mutex examples build/run/reject as
  specified.


# 7. Workstream Z: P4 — Eventuals

Goal: `| async` spawns and yields an eventual; `| await` blocks for its value;
error handling is identical to the synchronous call site.

## Z1. Async and await

Surface:

- `call() | async` spawns `call()` and yields an **eventual**
- `evt | await` blocks for the value
- the call target is a direct named routine declaration and may use either an
  unqualified name or a qualified path; stored routine values and routine
  parameters are rejected as indirect task targets, and receiver-method call
  syntax is not accepted as a task target
- the eventual type is **internal** in V3 — **not** user-nameable; `evt[T]` as a
  nameable type is an explicit later slot

Work:

- typecheck: delete the async/await rejections (`exprs/mod.rs:225,229`,
  `exprs/operators.rs:41,51`); `x | async` types `x` as spawned and yields an
  internal eventual over `x`'s value type; `e | await` types as the eventual's
  value type; an eventual value may only be produced by `| async`, moves between
  plain bindings/assignments rather than cloning, and may be consumed at most
  once by `| await` (it has no spellable type); a recoverable eventual must be
  consumed and handled, while an infallible eventual may remain for the
  process-exit join; composite embedding and unchecked generic-parameter
  crossings remain rejected in V3
- typecheck: reject implicit capture of an outer local by a nested routine, and
  reject `| await` or access to an existing eventual binding inside `edf`
- thread-safe move-only result values are transferred through the eventual and
  out of `| await` without cloning; the ordinary spawn boundary still rejects
  borrowed values and `ptr[shared, T]`
- lowering/backend: `| async` emits a `std::thread::spawn` returning its value
  through a join handle (or a one-shot channel); `| await` emits the join/recv
  that blocks for the value; process exit joins all outstanding eventuals,
  including infallible eventual bindings that are never awaited, so they are
  not detached

## Z2. Error transparency

The await site behaves **exactly** like the synchronous call site for recoverable
errors: the existing pipe-or `||` and `check(...)` handlers work with **zero**
new error surface. Current V1 deliberately has no plain `/` propagation and
rejects postfix `x!` on recoverable calls; await preserves those same boundaries
rather than inventing processor-only handling.

A recoverable eventual carries a must-handle obligation. Its final owner must be
awaited and the resulting recoverable value handled immediately with
`check(...)` or `||`. Transfer moves the obligation to the destination. Lexical
fallthrough and any `break`, `return`, or `report` that exits a scope with a live
obligation are rejected; at a branch join all continuing paths must leave a
compatible state by preserving the same owner, transferring consistently, or
discharging it. Consuming or transferring on only some paths is rejected.
Standalone discard, overwriting a live recoverable eventual, and discarding the
awaited result are also rejected. Infallible eventuals carry no such error
obligation and may rely on join-at-exit.

Work:

- typecheck: an awaited recoverable call carries its error type through `| await`
  unchanged; track the must-handle obligation through bindings, transfers,
  branches, and scope/routine exits; reject discard and overwrite; verify both
  current recoverable-call handlers (`||` fallback and `check(...)`) work
  identically on an awaited value
- lowering/backend: the awaited result is the same recoverable shell the
  synchronous call would produce; no new wrapper

Editor / tree-sitter for Z:

- tree-sitter: `| async` / `| await` already parse — verify the stage node
  shapes; add highlights for the `async`/`await` pipe keywords; validate zero
  ERROR nodes
- LSP hover on `| async` shows "spawns; yields an eventual (internal type)";
  hover on `| await` shows the awaited value type; hover shows the same error
  type and must-handle rule as the synchronous call
- LSP diagnostics reject bare recoverable async discard, live overwrite,
  unhandled scope exit, and discarded recoverable await results with the same
  compiler-owned codes as command-line checking

Original milestone seed examples (the complete current matrix is the
[canonical shipped inventory](../book/src/900_processor/_index.md#shipped-example-inventory)):

- positive: `examples/proc_async_await_m4` — `call() | async` then `evt | await`
- positive: `examples/proc_await_error_m4` — awaited recoverable calls handled
  with both `check(...)` and `||`, identical to the synchronous form
- negative: `examples/fail_proc_evt_named_m4` — attempting to name the eventual
  type (`var e: evt[int] = ...`) is rejected (internal-only)
- negative: `examples/fail_proc_async_in_core_m4` — `| async` in a `core` build
  (tier)
- negative: `examples/fail_proc_async_recoverable_discard_m4` — a recoverable
  async expression cannot be discarded as a statement
- negative: `examples/fail_proc_async_recoverable_unawaited_m4` — a bound
  recoverable eventual cannot leave scope unhandled
- negative: `examples/fail_proc_async_recoverable_overwrite_m4` — a live
  recoverable eventual cannot be overwritten
- negative: `examples/fail_proc_await_recoverable_discard_m4` — an awaited
  recoverable result still requires immediate `check(...)` or `||` handling
- negative: `examples/fail_proc_async_break_outer_m4` — `break` cannot leave a
  live recoverable eventual obligation in an exited loop scope
- negative: `examples/fail_proc_async_edf_await_m4` — `edf` cannot await or
  access an existing eventual binding
- negative: `examples/fail_proc_async_nested_capture_m4` — a nested routine
  cannot implicitly capture an outer eventual binding

Docs for Z: see Workstream AA (eventuals chapter: internal eventual,
must-handle error transparency, join-at-exit).

Tracked slices:

- [x] Z1. `| async` / `| await` with an internal (non-nameable) eventual type;
  delete the async/await rejections; join-all-at-exit.
- [x] Z2. Error transparency: recoverable eventual obligations are checked at
  continuing branch joins and every modeled lexical/routine exit, and both
  current handlers work identically on an awaited value, with no new surface.
- [x] Z3. LSP + tree-sitter: async/await hover/diagnostics, keyword highlights.
- [x] Z4. Positive and `fail_*` eventual examples build/run/reject as specified.


# 8. Workstream AA: Book Updates Required (Processor Pillar)

At planning time the V3 processor chapters were future-design sketches that
contradicted the decisions above in many places. This workstream rewrote them to
match in the same change set as the milestone that owned each fact.

Contradictions to fix (chapter -> exact edit):

- `book/src/900_processor/200_corutines.md`
  - the channel example uses `channel[rx][0]` **seq-index** receive — this model
    is **dead**; replace with the blocking pull expression `var x = c[rx]` and
    the `for msg in c[rx]` iteration (X2)
  - `chn[str]` is described as buffered with "four buffer transmitters" — replace
    with the unbounded-MPSC, closes-on-last-`tx`-drop model (X1)
  - the mutex section uses the `(( ... ))` **double-paren** parameter form and
    describes `((meshes))` — replace with `name[mux]: T` (Y2); keep `.lock()` /
    `.unlock()` and auto-unlock wording
  - the `select(channel as c) { sequence.push(channel[rx][c]) }` single-channel
    form is **dead**; replace with the multi-arm `select { when c as x { ... } }`
    form (Y1)
  - `~var` prefix usage stays (it is the memory pillar's `var[mut]` sugar); no
    `((...))` mutex spelling remains
  - the worker wording ("a worker takes the task") is reframed as thread-per-
    spawn with join-at-exit, with no worker-pool promise
- `book/src/900_processor/100_eventuals.md`
  - the async/await example is kept in spirit but reframed: `| async` yields an
    **internal** eventual (not user-nameable), `| await` blocks, and error
    handling is identical to the synchronous call, including the must-handle
    obligation for recoverable eventuals (Z2)
  - remove any implication of an async runtime, continuations, or invisible
    thread scheduling beyond "spawns an OS thread and joins at exit"
  - note the `evt[T]`-as-nameable-type future slot explicitly
- both chapters: state the `std`-only tier requirement up front (W1)
- `plan/VERSIONS.md` — the V3 section is expanded to describe the landed
  processor subset (additive; shared with the memory pillar's VERSIONS edit)

Tracked slices:

- [x] AA1. Rewrite `200_corutines.md` (dead `[rx][i]`, dead `((x))`, dead
  single-channel `select`, MPSC/unbounded, thread-per-spawn wording).
- [x] AA2. Rewrite `100_eventuals.md` (internal eventual, error transparency, no
  async runtime).
- [x] AA3. State the `std`-only tier requirement in both chapters.


# 9. Workstream BB: Tooling and Editor Hardening (Cross-Cutting)

This is not a phase after W through AA. It runs **in the same change set** as
each workstream above.

Per-feature editor requirements:

- positive LSP test: open each new positive example, run hover and
  go-to-definition over the new processor surface, assert sensible results
- negative LSP test: open each new `fail_*` example, assert the diagnostic text
  matches the new tier / `O`-code / boundary wording
- tree-sitter parse test: each new positive example parses to the expected node
  shape with zero ERROR nodes
- tree-sitter parse test: each removed/dead surface (single-channel `select(...)`,
  the `((name))` double-paren mutex param) **fails to parse** at the grammar
  level
- formatter test: every positive processor example remains idempotent and
  compiler-analyzable after formatting; comments and raw strings containing
  processor syntax do not affect structural formatting
- tool-command test: `fol tool parse`, `highlight`, and `symbols` execute the
  generated parser and shipped queries rather than approximating V3 syntax
- inventory test: the canonical positive/failure matrix stays identical to the
  checked-in `proc_*` and `fail_proc_*` package directories
- capability-routing test: direct and routed editor/frontend analysis use the
  evaluated artifact model and active bundled-standard dependency set

Primary files:

- `lang/tooling/fol-editor/tree-sitter/grammar.js`
- `lang/tooling/fol-editor/queries/fol/highlights.scm`
- `lang/tooling/fol-editor/queries/fol/locals.scm`
- `lang/tooling/fol-editor/queries/fol/symbols.scm`
- `lang/tooling/fol-editor/src/tree_sitter.rs`
- `lang/tooling/fol-editor/src/lsp/tests/example_models.rs`
- `lang/tooling/fol-editor/src/lsp/tests/navigation.rs`
- `lang/tooling/fol-frontend/src/`
- `lang/compiler/fol-diagnostics/src/`
- `test/v3_example_inventory.rs`
- any fixtures under `lang/tooling/fol-editor/tests/`

Tracked slices:

- [x] BB1. Editor / tree-sitter updates shipped alongside W (spawn sigil,
  hover/diagnostics).
- [x] BB2. Editor / tree-sitter updates shipped alongside X (endpoint keywords,
  index-rejection).
- [x] BB3. Editor / tree-sitter updates shipped alongside Y (multi-arm select,
  `[mux]` param, dead-form rejection).
- [x] BB4. Editor / tree-sitter updates shipped alongside Z (async/await keyword
  highlights, hover).

Rule:

- BB is listed separately only for plan visibility
- do not use it as an excuse to ship compiler changes first and editor changes
  later


# 10. Recommended Order

1. Workstream W (P1: spawn + the `std`-only tier gate). Establishes the gate the
   rest reuse.
2. Workstream X (P2: channels). Depends on spawn for producers.
3. Workstream Y (P3: select multiplexing + mutex). Depends on channels; mutex
   reuses the memory pillar's `[options]` seam and is the sanctioned cross-thread
   sharing mechanism.
4. Workstream Z (P4: eventuals). Depends on spawn; adds the internal eventual and
   error transparency.

Workstream AA (book) and Workstream BB (editor/tree-sitter) run **alongside**
each of the above in the same change set, never deferred.

The processor pillar begins only after the memory pillar (`plan/V3_MEM.md`)
completes fully.


# 11. Non-Goals Restated

Permanently out of scope for the processor pillar (some are named future slots,
not promises):

- worker pools / scheduling knobs (thread-per-spawn only in V3)
- Rust async / futures / tokio (no colored functions, no async runtime)
- bounded / buffered channels (`chn[T, N]` is a named future slot), MPMC,
  back-pressure
- cancellation of running tasks
- a user-nameable eventual type (`evt[T]` stays internal in V3; nameable is a
  named future slot)
- cross-process / network channels
- sharing `Rc` or any non-thread-safe value across a spawn boundary (use `[mux]`)
- resumable generators/coroutines and executable `yield`; the parser retains a
  `yield` node, but typecheck and lowering reject it in V3

If a workstream starts to require one of these, it stops.


# 12. Completion Rule

The processor pillar is complete only when:

- P1 (W), P2 (X), P3 (Y), and P4 (Z) each ship end to end in parser, resolver,
  typecheck, lowering, runtime/backend, frontend routing, structured
  diagnostics, formatter/tool commands, LSP, tree-sitter grammar/queries/corpus,
  docs, book, and canonical examples, and each landed workspace-green
- the whole processor surface is `std`-only, verified by tier `fail_*` examples
  in `core` and `memo`
- no "planned for a future release" message in the compiler refers to a processor
  surface the project has chosen to build
- the dead forms are gone: single-channel `select(...)` and `((name))` mutex
  parameters no longer parse
- backend emission uses only `std::thread` / `std::sync::mpsc` /
  `std::sync::Mutex` / `std::sync::Arc`, with process exit joining all tasks
- the V3 processor chapters (`100_eventuals.md`, `200_corutines.md`) describe the
  resulting language exactly, with no leftover pre-rewrite wording
- the project can honestly say: "V3 concurrency is OS threads, unbounded MPSC
  channels, mutex-guarded sharing, and internal eventuals — `std`-only, with no
  async runtime, no worker pool, and no unsafe cross-thread `Rc`"
