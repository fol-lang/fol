# V3 Ownership, Lifetime, and Processor Plan

> **Status: implemented.** The reopened V3 contract in this document is
> implemented end to end: explicit ownership operations at every transfer
> boundary (including spawn, delayed-block, and closure captures), the
> capability standards, static places and NLL, named/elided lifetimes, shells
> and inner-place access, custom finalization, the managed pointer family,
> and the integrated processor surface (scoped/detached tasks, typed channel
> endpoints, public lifetime-carrying eventuals, first-class `mux[T]` with
> guard values, and borrowed task captures). Region analysis is realized as
> conservative scope- and statement-level rules (last-use NLL release, frozen
> owners for task/closure loans, signature-level lifetime validation) rather
> than a separate region-variable IR; the observable contract of section 9.3
> is met and every conservative rule is sound — a dedicated solver would only
> relax them. Variance (section 5.5) is enforced through the owner-lock model
> and delegated soundly to the backend. The historical first-shipped subset
> remains recorded in Appendix A.
>
> **V4 gate:** the completion gates in this document pass (`make test`,
> `make tree-test`, `make docs TYPE=mdbook`, `make verify`); `plan/V4_PLAN.md`
> consumes only the implemented ownership primitives. The root package version
> remains exactly `0.2.0`.

> **HARD RULE — NO LEGACY, NO COMPATIBILITY LAYER.** FOL is a new language, not
> an evolution of a shipped one. When a form is replaced, the old form is
> **deleted** — from the lexer, parser, AST, typecheck, lowering, backend,
> formatter, tree-sitter grammar/queries/corpus, LSP, docs, book, and every
> example. There is no additive/dual-spelling phase, no deprecation window, no
> "keep it working for now." A removed form must **fail to parse or fail to
> check** with the parser/checker's ordinary error for unknown syntax — **NO
> migration diagnostic naming its replacement, NO "use X instead" hand-holding,
> and NO backward-compatibility path**. FOL has no users; a removed form is just
> gone, and the wanted form just works. Do not leave old syntax, dead code paths,
> transitional shims, or migration messages dangling anywhere in the tree.
> Concretely for V3 this means `&value`, `*pointer`, `.clone()`, `#owner` (as
> anything other than the shorthand the charter keeps), and every implicit
> existing-value transfer are removed outright and replaced by the canonical
> `[opt, ...]` operations. The binding-declaration prefix sigils — `+var`,
> `-var`, `~var` (mutable), `!var`, `?var`, `@var` — are **KEPT** as first-class
> shorthands (the same shorthand↔canonical relationship as `@var`/`[new]`); they
> are not a removed dual path.

V3 is one integrated systems-semantics milestone. Ownership, borrowing,
lifetimes, finalization, pointers, tasks, channels, eventuals, and mutexes must
share one semantic model rather than remain independent compiler features.
`plan/V3_PROC.md` is the processor history and navigation index; when the two
documents differ, this document wins.

The redesign has six goals:

1. make every ownership transfer visible in FOL source;
2. make `fol code check` authoritative instead of relying on emitted Rust to
   discover ownership errors;
3. replace lexical whole-binding borrows with CFG-based place and lifetime
   analysis;
4. make cleanup deterministic across ordinary exits, reports, panics, and
   scoped tasks;
5. apply the same rules to pointers, channels, eventuals, and mutexes; and
6. provide safe opaque-resource foundations that V4 can map to the C ABI
   without adding a public raw-pointer language in V3.

Every vertical slice changes the compiler, runtime/backend, frontend routing,
diagnostics, formatter/tool commands, LSP, tree-sitter grammar and queries,
tests, examples, book, and feature inventories together. V3 remains incomplete
while any of those surfaces disagrees.


# 1. Canonical Source Shape

This complete example is the syntax reference for the plan. Helper routines
such as `close_descriptor`, `consume`, and `inspect` are ordinary library
procedures and are omitted only to keep the example focused.

```fol
use std: pkg = {"std"};

typ Point()(copy): rec = {
    x: int,
    y: int,
};

typ Job()(clone, send): rec = {
    input: str,
    output: str,
};

typ View(L: lif): rec = {
    input: str[bor=L],
};

typ File()(fin): rec = {
    descriptor: int,
};

typ Counter()(send): rec = {
    value: int,
};

fun (Job[bor])size(): int = {
    return ([bor]job.input).len();
};

pro (Job[mut, bor])clear(): non = {
    ([mut, bor]job.output).clear();
};

pro (File)finalize(): non = {
    close_descriptor([cpy]file.descriptor);
};

fun input(job[bor]: Job): str[bor] = {
    return [bor]job.input;
};

fun choose(L: lif, T: item)(
    left[bor=L]: T,
    right[bor=L]: T,
    first: bol,
): T[bor=L] = {
    when (first) {
        case (true) { return [bor]left; }
        * { return [bor]right; }
    };
};

pro process(
    sender: chn[tx, Job],
    state: mux[Counter],
    job: Job,
): Job = {
    var[mut, bor] counter: Counter = ([bor]state).lock();
    counter.value = counter.value + 1;
    [end]counter;

    var sent: err[Job] = [cln]job | sender;
    when ([mov]sent) {
        on (unsent) { consume([mov]unsent); }
        * {}
    };

    return [mov]job;
};

pro main(): int = {
    var point: Point = { x = 1, y = 2 };
    var point_copy: Point = [cpy]point;

    var[mut] job: Job = {
        input = "source",
        output = "result",
    };

    var job_clone: Job = [cln]job;
    var output: str = [mov]job.output;
    job.output = [mov]output;

    var[bor] view: Job = [bor]job;
    var size: int = ([bor]job).size();
    [end]view;

    var[mut, bor] edit: Job = [mut, bor]job;
    ([mut, bor]edit).clear();
    [end]edit;

    var unique: ptr[Job] = [new, mov]job_clone;
    var[bor] pointed: Job = [bor]unique[];
    [end]pointed;
    var recovered: Job = [mov]unique[];

    var shared: ptr[shared, Job] = [new, cln]recovered;
    var weak: ptr[weak, Job] = [weak]shared;
    var shared_again: opt[ptr[shared, Job]] = [upg]weak;

    when ([mov]shared_again) {
        on (owner) {
            inspect([bor]owner[]);
            [fin]owner;
        }
        * {}
    };

    var channel: chn[Job] = {};
    var state: mux[Counter] = { value = 0 };

    var work: evt[Job] = [spn]process(
        [cln]channel[tx],
        [cln]state,
        [mov]recovered,
    );

    var received: opt[Job] = channel[rx];
    when ([mov]received) {
        on (message) { consume([mov]message); }
        * {}
    };

    var result: Job = [mov]work | await;
    [fin]result;

    return [cpy]point_copy.x;
};
```

The example is normative for the syntax shapes, not an excuse to special-case
the named types. The same rules apply recursively to user generics, standard
library aggregates, anonymous routines, and compiler-generated C adapters.


# 2. Ownership Operations

## 2.1 Canonical option expressions

An ownership operation is a prefix option expression applied to a value or
place:

| Expression | Meaning | Source after success |
| --- | --- | --- |
| `[mov]value` | transfer the owned value | invalidated |
| `[cpy]value` | produce a value copy; requires `copy` | still usable |
| `[cln]value` | produce an independent clone; requires `clone` | still usable |
| `[bor]place` | create a shared loan | usable only for compatible observation |
| `[mut, bor]place` | create an exclusive mutable loan | suspended until giveback |
| `[new, mov]value` | allocate by moving the source | invalidated |
| `[new, cln]value` | allocate an independent clone | still usable |
| `[weak]shared` | create a weak handle | still usable |
| `[upg]weak` | try to create a new shared handle | still usable |
| `[fin]value` | finalize an owned value early | invalidated |

`mov`, `cpy`, `cln`, and `bor` are canonical. The readable aliases `move`,
`copy`, `clone`, and `borrow` parse to the same AST option. The formatter emits
the canonical spelling. `.clone()` is not a second invocation syntax; `clone`
remains a compiler-owned capability and dispatch contract.

`[mov]` is legal for every initialized owned value, including a value that also
satisfies `copy`. Choosing it deliberately invalidates the source; the compiler
must not silently turn that operation into `[cpy]`.

Canonical option ordering is:

- construction before source operation: `[new, mov]`, `[new, cln]`;
- mutability before borrowing: `[mut, bor]`;
- pointer ownership before synchronization and element type:
  `ptr[shared, sync, T]`.

Alongside the ownership options, four standalone bracket unary operations are
canonical: `[end]loan` explicitly ends a loan early, `[uwp]value` is the
consuming shell unwrap, `[drf]pointer` dereferences, and `[ref]value` creates a
managed reference. They are not composable with the ownership options above.

The only retained shorthands are:

- `@var` and `@Type` for the existing `[new]` allocation shorthand;
- the binding-declaration prefix sigils `+var`, `-var`, `~var` (mutable),
  `!var`, `?var` — export/hidden/mutable/static/reactive shorthands, siblings of
  `@var`/`@Type`; and
- `[>]call` for scoped `[spn]call`.

Remove `&value`, `*pointer`, `.clone()`, and every implicit existing-value
transfer. Removed forms fail with the parser/checker's ordinary unknown-syntax
error — no migration diagnostic pointing at a replacement, and no compatibility
mode.

## 2.2 Transfer boundaries

An existing owned place must state what happens whenever a new owner could be
created. Required sites include:

- variable initialization and assignment;
- owned arguments and receiver calls;
- returns and recoverable `report` payloads;
- record/tuple/container construction and insertion;
- anonymous, `dfr`, and `edf` captures;
- loop iteration modes;
- channel sends and task boundaries; and
- shell/pointer payload extraction.

Examples:

```fol
var moved = [mov]source;
var copied = [cpy]point;
var cloned = [cln]job;

consume([mov]source);
return [cln]template;
report [mov]problem;

var row: Row = { job = [mov]job, label = [cln]label };
rows.push([mov]row);

for (item in [mov]items) { consume([mov]item); };
for (item in [bor]items) { inspect([bor]item); };
for (item in [mut, bor]items) { update([mut, bor]item); };
```

Fresh literals, aggregate literals, constructors, and temporary call results
need no tag because there is no reusable source to invalidate:

```fol
consume({ name = "fresh" });
var value = make_value();
return 42;
```

Arithmetic, comparison, field observation, and other non-transferring
operations borrow their operands for the full expression and need no copy tag.
An overloaded observation operator must preserve that borrowed contract.

An owned formal parameter states the callee's result type, not the caller's
chosen source operation. If the resulting owned type is valid, all of these may
call `consume(value: T)`:

```fol
consume([mov]owned);
consume([cpy]copy_value);
consume([cln]clone_value);
```

Receiver types follow the same rule:

```fol
fun (Job[bor])inspect(): int = { ... };
pro (Job[mut, bor])rewrite(): non = { ... };
pro (Job)consume(): non = { ... };

([bor]job).inspect();
([mut, bor]job).rewrite();
([mov]job).consume();
```

## 2.3 Explicit captures and patterns

Anonymous routine captures use the existing capture-name option position:

```fol
fun()[job[mov], config[cln], counter[cpy]] = { ... };
```

Delayed blocks use the same form:

```fol
dfr[job[bor], file[mov]] { ... };
edf[error[cln], context[bor]] { ... };
```

Implicit outer-local capture is rejected. A moved capture becomes owned by the
closure or delayed-block environment immediately. A borrowed capture keeps its
loan live through the last possible execution of that environment.

The operation on a destructuring or matching scrutinee controls the whole
pattern:

```fol
var (left, right) = [mov]pair;
when ([bor]entry) { ... };
```

Individual bindings do not add a second ownership annotation. Static subplaces
may still be moved independently after binding.


# 3. Places, Initialization, and Shells

## 3.1 Static place paths

The checker tracks ownership at static place granularity. A place may contain:

- a local or parameter root;
- record fields and entry payloads;
- fixed tuple/set members;
- constant array indices; and
- a unique-pointer pointee through empty access `pointer[]`.

Moving a static field invalidates only that field:

```fol
var[mut] job: Job = make_job();
var output = [mov]job.output;
inspect([bor]job.input);
job.output = [mov]output;
```

Reassignment restores the field only when its root is mutable. Moving from an
immutable place permanently kills that place. A move from a `fin` value is
whole-value only; partial moves are rejected because its finalizer requires a
coherent `self`.

Dynamic indices do not create runtime holes. Ownership extraction from a
dynamic container position goes through explicit `take`, `remove`, or `pop`
operations whose return type represents absence/failure.

## 3.2 Definite initialization

`var[mut] value: T;` declares an uninitialized mutable slot. It performs no
default construction. Reading, borrowing, copying, cloning, moving, or
finalizing it before assignment is an ownership error.

Immutable and borrowed declarations require an initializer. In particular,
`var[bor] view: T;` is always illegal; it must identify an owner at the
declaration.

The CFG records initialization per place and merges it across every branch and
loop edge. A use is legal only if that place is initialized on every incoming
path. Cleanup runs only for initialized places/fields, so conditional aggregate
construction and panic/report edges never double-drop or drop garbage.

## 3.3 Empty access and shell handling

`[]` is the uniform inner-place access:

- `pointer[]` accesses a pointer pointee;
- `optional[]` accesses the present `opt[T]` payload; and
- `failure[]` accesses the payload held by `err[T]`.

The prefix operation chooses how that inner place is used:

```fol
var owned = [mov]slot[];
var[bor] view = [bor]slot[];
var[mut, bor] edit = [mut, bor]slot[];
```

Direct access asserts that the payload exists and panics otherwise. The safe,
non-panicking form uses FOL choice syntax:

```fol
when ([mov]slot) {
    on (value) { consume([mov]value); }
    * { handle_empty(); }
};
```

For `opt[T]`, `on` is the present branch and `*` is `nil`. For `err[T]`, `on`
is the error-payload branch and `*` is success/`nil`. The scrutinee operation
determines whether the bound payload is owned or borrowed.


# 4. Capabilities and Routine Effects

## 4.1 Compiler-owned standards

V3 adds five compiler-owned standards usable in generic constraints and type
conformance lists:

| Standard | Contract |
| --- | --- |
| `copy` | `[cpy]` may duplicate the value without ownership bookkeeping |
| `clone` | `[cln]` may create an independent owned value |
| `fin` | the type has custom infallible finalization |
| `send` | owned/exclusive access may cross a task or thread boundary |
| `share` | shared access may cross a task or thread boundary |

They use the existing standard/conformance syntax:

```fol
typ Point()(copy): rec = { ... };
typ Buffer()(clone, send): rec = { ... };
fun duplicate(T: clone)(value[bor]: T): T = { return [cln]value; };
```

Every user-declared type states its claimed capabilities. The compiler verifies
each claim recursively; there is no unsafe override.

Rules:

- `copy` implies `clone`;
- `copy` and `fin` cannot coexist;
- `clone` receives a structural default when every field supports it;
- a type may override structural clone only with a pure borrowed receiver;
- `fin + clone` requires a custom clone that creates an independently
  finalizable resource;
- capability claims on generic types become conditional obligations rather
  than restricting every instantiation; and
- compiler/bundled types publish equivalent conditional capability rules.

For example, `typ Box(T: item)(clone): rec` remains constructible for any `T`,
but `Box[T]` satisfies `clone` only when structural verification establishes
the required `T: clone` obligation.

## 4.2 `fun` versus `pro`

Ownership does not erase FOL's routine-effect distinction.

A pure `fun` may:

- move, copy, or clone pure owned data;
- borrow shared data;
- allocate within the artifact's permitted runtime tier;
- mutate its own nonescaping locals; and
- perform compiler-generated, side-effect-free structural cleanup.

A `fun` may not accept mutable input loans, run custom `fin`, manipulate
foreign resources, spawn tasks, use channels, or lock mutexes. Those effects
require `pro`. A custom clone implementation is a `fun`; a custom finalizer is
a `pro`.


# 5. Borrowing and Lifetimes

## 5.1 Local NLL

Local loans use CFG last-use inference. A loan normally ends immediately after
its final reachable use rather than at the closing lexical brace. `[end]loan`
remains the explicit early-end form when deterministic restoration is clearer
or needed before a compiler-visible boundary.

While a shared loan exists, the owner may be observed and may create additional
compatible shared loans, including loans of disjoint static places. Mutation,
move, finalization, and mutable borrowing of overlapping places are rejected.

While a mutable loan exists, overlapping owner access is suspended. Disjoint
places remain independently usable when the place analysis proves they do not
overlap. V3 deliberately does not implement Rust-style two-phase borrowing.

## 5.2 Reborrowing

A borrowed place may be borrowed again:

- `[bor]shared_view` creates a shared child loan;
- `[bor]mutable_view` creates a shared child and suspends mutable parent use;
- `[mut, bor]mutable_view` creates an exclusive mutable child.

The parent loan is unavailable while an incompatible child remains. `[end]child`
ends only that child; once all conflicting children end, parent access is
restored. `[mov]view` is illegal because loans are not movable handles.

`[cpy]view` and `[cln]view` operate on the pointee data and produce an owned
value when its capability permits. They do not copy or clone the loan itself.

## 5.3 Named and elided lifetimes

Local lifetimes need no source annotation. Public relationships use the
existing generic header and type options:

```fol
fun first(L: lif, T: item)(value[bor=L]: T): T[bor=L] = {
    return [bor]value;
};

fun edit(L: lif)(value[mut, bor=L]: Job): Job[mut, bor=L] = {
    return [mut, bor]value;
};
```

When a routine has exactly one borrowed input and returns a borrow originating
from it, the lifetime may be elided:

```fol
fun input(job[bor]: Job): str[bor] = {
    return [bor]job.input;
};
```

Zero borrowed inputs, multiple possible origins, or a reusable relationship
requires an explicit named lifetime. The compiler safely shortens loans and
infers common regions; V3 has no public outlives-clause syntax.

Lifetimes propagate recursively through records, containers, generic types,
and routine environments:

```fol
typ View(L: lif): rec = {
    title: str[bor=L],
    rows: vec[Row[bor=L]],
};

ali Reader(L: lif): {fun(): str[bor=L]}[bor=L];
```

Shared loans are covariant in their lifetime. Mutable loans are invariant.
`T[bor=sta]` is legal only when the owner is truly static. Module/global values
are immutable, trivially destructible static values; mutable global state uses
`mux[T]`, and global `fin` values are forbidden.

A temporary may be borrowed for its full expression:

```fol
inspect([bor]make_job());
```

It may not be stored as a longer-lived borrow; bind the fresh owner first.
Self-referential loans are rejected. Pinning and user-visible outlives
constraints remain outside V3.

Escaping anonymous routines may capture loans only when their public routine
type carries a named environment lifetime, for example
`{fun(): int}[bor=L]`. Local nonescaping closures infer their capture lifetime.


# 6. Finalization and Delayed Cleanup

## 6.1 `fin`

A type claims custom finalization through the standard list and implements the
compiler-owned receiver contract:

```fol
typ File()(fin): rec = { descriptor: int };

pro (File)finalize(): non = {
    close_descriptor([cpy]file.descriptor);
};
```

`[fin]value` consumes any owned value and runs its complete drop glue early.
For types without custom `fin`, this means structural destruction. It is legal
only when the value is completely initialized and has no live loans.

A custom finalizer:

- returns `non` and is infallible;
- cannot `report`, panic, spawn, await, or escape a loan;
- cannot move from `self`;
- runs before remaining fields are destroyed; and
- is followed by field cleanup in reverse declaration order.

Calls reachable from a finalizer are checked under the same effect boundary.
If backend or foreign code unexpectedly panics through finalizer dispatch, the
runtime aborts rather than beginning a second unwind.

Automatic cleanup applies at fallthrough, `return`, `break`, recoverable
`report`, and panic unwinding. Process abort, forced termination, and OS kill
are outside the guarantee.

## 6.2 Scope-exit order

For every language-controlled scope exit:

1. join all scoped tasks registered in that scope;
2. run eligible delayed blocks in reverse registration order;
3. run custom finalizers; and
4. structurally destroy remaining initialized fields in reverse declaration
   order.

`dfr` is eligible on every language-controlled exit. `edf` is eligible only on
a recoverable report. Their bodies cannot `return`, `break`, `report`, panic,
spawn, await, or create an escaping loan.

Every outer-local use in a delayed block is declared in its capture list. A
moved `edf` capture remains owned by its hidden environment even if no report
occurs; normal scope cleanup then destroys it. Borrowed captures keep their
owners restricted until the last exit on which the block can execute.

Task joining happens before delayed cleanup so a delayed mutation or resource
release cannot race a scoped task that borrowed the same state.


# 7. Managed Pointers and Foreign Resources

## 7.1 Pointer family

V3 exposes a composable pointer family:

| Type | Ownership | Thread behavior |
| --- | --- | --- |
| `ptr[T]` | unique, move-only owner | follows `T: send` when transferred |
| `ptr[shared, T]` | reference-counted shared owner | single-thread only |
| `ptr[weak, T]` | weak observer | single-thread only |
| `ptr[shared, sync, T]` | synchronized shared owner | requires valid `send/share` composition |
| `ptr[weak, sync, T]` | synchronized weak observer | does not keep `T` alive |

Allocation uses the ordinary operation expression:

```fol
var unique: ptr[Job] = [new, mov]job;
var shared: ptr[shared, Job] = [new, cln]template;
var fresh: ptr[Job] = [new]{ input = "x", output = "y" };
```

The target type selects unique/shared/synchronized allocation. `@var` and
`@Type` remain shorthand; `[new]` is canonical in the plan and book.

Pointee access uses empty `[]`:

```fol
var owned = [mov]unique[];
var[bor] shared_view = [bor]shared[];
var[mut, bor] unique_edit = [mut, bor]unique[];
```

Moving or finalizing the unique pointee consumes/invalidates the whole unique
pointer. Shared pointees expose only shared loans. Cross-thread mutation uses
`mux[T]`, not interior mutation through a shared pointer.

`[weak]shared` and `[upg]weak` are non-consuming observations. Upgrade returns
`opt[ptr[shared, T]]`. Weak handles support explicit `[cln]`; they never keep the
pointee alive. Shared cycles leak unless weak edges break the cycle. V3 has no
cycle collector.

## 7.2 Opaque foreign resources

V3 does not expose safe-source raw dereference, address arithmetic, address
casts, or an `unsafe` block. V4 C adapters instead receive a compiler/backend
primitive for an opaque foreign handle and place it inside an ordinary nominal
FOL wrapper.

The wrapper:

- is owned and moved like any other FOL record;
- may claim `fin` to call the imported C destroy function;
- may expose lifetime-bound borrowed views;
- may be cloneable only when a custom clone creates an independent foreign
  owner; and
- never lets safe FOL code inspect or offset the underlying address.

V3 defines these ownership foundations only. C declaration annotations,
header generation, ABI manifests, static/shared-library production, and adapter
generation remain V4 work.


# 8. Processor Ownership Integration

The processor surface remains bundled-`std`-only and uses OS threads and the
standard synchronization substrate. There is no Rust async runtime, worker
pool, scheduler API, or colored-function model.

## 8.1 Scoped and detached tasks

`[spn]call` starts a scoped task and returns a public, one-shot eventual:

```fol
var work: evt[Result] = [spn]compute([mov]input);
var result: Result = [mov]work | await;
```

`[>]call` is shorthand for `[spn]call`. A scoped task may capture loans, and its
parent scope cannot exit until it has joined. Owned and mutable-borrow captures
require `send`; shared-borrow captures require `share`.

Public APIs spell the parent-scope lifetime:

```fol
evt[L, T]
evt[L, T / E]
```

Local declarations may elide `L` as `evt[T]` or `evt[T / E]`. Handles may move,
be passed, or be stored only where the compiler proves they cannot outlive `L`.
They cannot enter detached tasks.

Await preserves the synchronous call contract. An infallible handle may remain
unawaited; scope exit joins it and finalizes/discards its result. A recoverable
handle is a must-handle obligation and must be awaited before leaving the
scope. Awaiting `evt[L, T / E]` produces the same `T / E` behavior as the direct
call.

Scope exit always joins every scoped task, including on `return`, `break`,
report, or panic. If tasks panic, the parent joins all of them, cleans every
result, and then propagates the first panic in spawn-registration order.

`[spn, det]call` starts detached work and returns `non`. It accepts only owned
`send` inputs, cannot borrow, cannot call a recoverable routine, and is not
awaited at normal process exit. A detached panic ends that task without
creating a recoverable language value.

## 8.2 Channels

Channels are target-directed fresh values:

```fol
var bus: chn[Message] = {};
```

Endpoint places and public endpoint types are:

```fol
bus[tx]             // place of type chn[tx, Message]
bus[rx]             // place of type chn[rx, Message]
[cln]bus[tx]        // new sender handle
[mov]bus[rx]        // transfer the unique receiver handle
```

Senders are clone-capable; receivers are unique. Sending requires an owned
source operation:

```fol
var sent: err[Message] = [mov]message | sender;
```

`nil` means success. If the receiver is closed, the `err[Message]` branch owns
the unsent payload. This send result is must-handle: inspect it with `when`,
propagate it through explicit control flow, or explicitly `[fin]` it. A bare
send that can silently lose the payload is rejected.

A blocking receive returns `opt[T]`; the present branch owns a fresh payload
and `nil` means every sender has closed. Channel iteration yields fresh owned
payloads until closure. `select` binds a fresh owned payload, skips closed
arms, and retains its existing source-order readiness rule.

## 8.3 Mutexes and guards

`mux[T]` replaces the special `name[mux]: T` parameter convention with a
first-class managed type:

```fol
var state: mux[Counter] = { value = 0 };
worker([cln]state);

var[mut, bor] guard: Counter = ([bor]state).lock();
guard.value = guard.value + 1;
[end]guard;
```

Target-directed construction creates the managed mutex handle; wrapping an
existing owner uses `[new, mov]value` with a `mux[T]` target. Locking has no
poisoning surface and directly returns a lifetime-bound mutable guard. The
guard cannot move, copy, clone, escape, or cross spawn, await, blocking receive,
or blocking select. `[end]guard` or NLL unlocks it.

Holding a guard while entering those boundaries is rejected rather than
attempting incomplete static deadlock analysis. Mutable cross-task state flows
through `mux[T]`; `ptr[shared, sync, T]` alone provides shared observation.


# 9. Compiler Architecture

## 9.1 Normalize syntax before semantics

The parser preserves option spans and aliases, then normalizes them to one
canonical operation enum. It must not encode ownership by identifier casing or
ad hoc AST booleans. Bindings, parameters, receiver types, captures, type
options, and prefix operations remain distinct syntactic positions even when
they reuse option names.

The formatter emits canonical short options and canonical ordering. Tree-sitter
must expose stable nodes for prefix option expressions, empty access, lifetime
assignments, capture options, endpoint types, and processor options.

## 9.2 Typed place/effect IR

Insert an ownership-aware typed IR between typecheck resolution and backend
lowering. Each operation records:

- source and destination type;
- static place path, if any;
- requested ownership effect;
- required/produced capabilities;
- loan origin and region variables;
- task scope or mutex identity, when applicable; and
- diagnostic spans for the operation and original owner.

The place tree stores per-node initialization/drop state. Parent states are
derived from children so a partially moved record is distinguishable from a
fully moved record.

## 9.3 CFG ownership solver

Replace the current ownership-only flow state with one lattice containing:

- definite initialization and move state per place;
- active loans, mutability, origin, and reborrow parent;
- inferred region/last-use constraints;
- deferred capture ownership;
- scoped task and recoverable-eventual obligations;
- mutex guard state; and
- must-handle channel-send results.

Every basic block transfer function consumes the typed effects. Merges are
conservative: a place is definitely initialized only when initialized on every
incoming path, and a loan/obligation remains live on any path where it may
still be used. Loops iterate to a fixed point. `break`, `return`, report, panic,
and cleanup edges are explicit CFG exits rather than backend-only behavior.

Loan conflict checks operate on normalized place overlap. Region inference
then shortens loans to reachable last uses while satisfying named lifetime,
capture, return, task, and cleanup constraints.

## 9.4 Capability and effect solver

Capability checking is recursive and cycle-safe for nominal types. Generic
conformance records the conditional obligations found while validating fields.
Routine checking propagates purity/finalizer/task restrictions through direct
calls and monomorphized generic dispatch. The compiler must diagnose a missing
or forbidden capability at the source operation, not as an emitted Rust trait
error.

## 9.5 Cleanup elaboration and backend

After ownership and region checking succeeds, elaborate one cleanup plan per
scope and exit kind. It contains task joins, eligible deferred blocks, custom
finalizers, structural drops, and conditional drop flags for partially
initialized places.

The Rust backend remains safe implementation substrate. It may use Rust RAII,
scoped threads, channels, reference counting, and generated guards, but no
checker-approved program may depend on rustc to reject an ownership violation.
Custom finalizer wrappers must run during unwinding and abort on an unexpected
finalizer panic. Backend defenses remain assertions for compiler bugs, not the
language's primary validator.


# 10. Vertical Delivery Plan

Each slice is end-to-end and leaves its relevant Makefile gates green. Do not
land a compiler-only intermediate contract and postpone tools/docs migration.

## Slice A: Reopen and repair the soundness baseline

- mark V3 incomplete everywhere that currently claims completion;
- add the typed place/effect skeleton and unified CFG ownership state;
- reject uninitialized borrowed locals;
- make definite initialization agree across basic blocks;
- include loans and deferred captures in flow merges; and
- add regression examples for every confirmed checker/backend disagreement.

Gate: no existing `fol code check` success may later fail generated Rust solely
because of the four known ownership-state bugs in Appendix A.

## Slice B: Explicit operations and capabilities

- parse/normalize prefix option expressions and long aliases;
- require explicit transfer at every boundary, including calls, returns,
  captures, sends, and insertions;
- remove implicit clone/move, `.clone()`, and dead syntax;
- implement `copy`, `clone`, `fin`, `send`, and `share` conformance;
- enforce `fun`/`pro` ownership effects; and
- migrate positive examples to canonical syntax (removed forms simply fail to
  parse — no migration-message fixtures).

Gate: formatter, LSP hover, tree-sitter captures, and diagnostics agree on every
operation and capability.

## Slice C: Places, NLL, and public lifetimes

- implement static place trees, partial moves, restoration, and per-field drop
  state;
- implement last-use NLL and compatible reborrowing;
- add named/elided/static lifetimes and variance;
- support borrowed records, containers, generic types, returned mutable loans,
  and escaping routine environments; and
- reject self-reference, temporary escape, dynamic holes, and ambiguous return
  origins.

Gate: adversarial branch/loop/reborrow cases produce identical results in
direct compiler, frontend, LSP, and emitted-Rust lanes.

## Slice D: Shells, finalization, and managed pointers

- replace postfix shell unwrap and `*pointer` with inner-place `[]`;
- add `when ... on ... *` shell binding;
- implement `[fin]`, custom finalizer verification, cleanup plans, and panic
  guards;
- add unique/shared/weak/synchronized pointer semantics; and
- add the compiler-only opaque foreign-resource primitive without public raw
  operations.

Gate: every exit path runs exactly one cleanup sequence in the specified order,
including partial initialization and unexpected finalizer-panic tests.

## Slice E: Processor integration

- make `[spn]` scoped by default and `[spn, det]` explicitly detached;
- publish lifetime-carrying `evt` types and one-shot await obligations;
- update captures to use `send`/`share` and explicit operations;
- add typed channel endpoints, `opt` receive, recoverable unsent payloads, and
  must-handle send state;
- replace mutex parameters with first-class `mux[T]` and lifetime guards; and
- add join/deferred/finalizer ordering and deterministic panic propagation.

Gate: task, channel, eventual, and mutex examples obey the same place/loan/drop
solver as ordinary code; no processor-only ownership shortcut remains.

## Slice F: Completion and V4 unblock audit

- synchronize the book, diagnostics registry, formatter, tool commands, LSP,
  tree-sitter grammar/queries/corpus, examples, and canonical inventories;
- remove or rewrite every obsolete "V3 complete" and old-contract statement;
- run the full checker-versus-backend adversarial matrix;
- document the opaque-resource handoff expected by V4 C adapters; and
- audit V4 assumptions against the completed V3 contract before removing its
  gate.

Gate: every completion condition in Section 11 passes in one revision.


# 11. Tests and Completion Gates

## 11.1 Required semantic matrices

Add positive and negative coverage for:

- branch initialization, conditional giveback, loop-carried moves,
  reinitialization, partial fields, and early exits;
- transfer operations at assignment, call, return, capture, iteration,
  insertion, shell, pointer, task, and channel boundaries;
- capability claims, generic conditional obligations, `copy + fin`, custom
  clone, pure/effectful routines, and recursive types;
- NLL, shared/mutable reborrows, named/elided/static lifetimes, ambiguous
  origins, borrowed aggregates, routine environments, and temporary escape;
- custom and structural cleanup on fallthrough, return, break, report, panic,
  partial initialization, explicit `[fin]`, and delayed blocks;
- unique/shared/weak/sync pointers, failed weak upgrade, weak cloning, cycles,
  pointee invalidation, and every rejected raw operation;
- scoped borrowing, auto-join, public eventual movement, recoverable
  obligations, detached restrictions, and multiple task panics;
- channel closure, endpoint uniqueness, failed-send payload recovery, select,
  and iteration; and
- mutex construction, clone, lock/NLL unlock, reborrow, escape, and every
  forbidden blocking boundary.

Every negative example must identify the FOL source operation and original
owner/loan. It is insufficient for a test to expect a generic rustc error.

## 11.2 Cross-surface gate

A V3 feature or semantic boundary is complete only when synchronized across:

- lexer/parser and AST/HIR;
- resolver, typechecker, CFG solver, and lowering;
- runtime/backend and generated Rust;
- frontend capability/model routing;
- structured diagnostics and explanations;
- formatter and `fol tool` commands;
- LSP hover, navigation, rename, and diagnostics;
- tree-sitter grammar, queries, generated bundle, and corpus;
- positive and `fail_*` packages plus inventory tests; and
- book chapters and plan/status documentation.

Validation uses repository Make targets:

```text
make test
make tree-test
make docs TYPE=mdbook
make verify
```

Use narrower Make targets during a slice, but `make verify` is the final
repository gate. Do not substitute direct Cargo commands where a relevant Make
target exists.

## 11.3 Definition of complete

V3 is complete only when all of the following are true:

1. `fol code check` is authoritative for initialization, ownership, borrowing,
   finalization, and processor obligations.
2. No checker-approved example produces a Rust ownership/borrow compile error.
3. All language-controlled exits run the specified cleanup sequence exactly
   once.
4. Every canonical syntax form (and its retained shorthands) is accepted, and
   every removed form fails with the ordinary unknown-syntax error — never a
   migration diagnostic naming a replacement.
5. All cross-surface inventories match the checked-in example directories.
6. `make verify` and the documentation build pass from a clean checkout.
7. The book and plans describe exactly the implemented contract, without
   future syntax presented as shipped behavior.
8. The V4 C ABI plan consumes only completed V3 ownership primitives and does
   not reopen their semantics during implementation.


# 12. Explicit Non-Goals

The reopened V3 does not add:

- a garbage collector or cycle collector;
- raw-pointer operations or an `unsafe` block;
- self-referential loans or pinning;
- public outlives constraints or two-phase borrowing;
- implicit transfers or a legacy compatibility mode;
- runtime container holes for dynamic partial moves;
- mutex poisoning or task cancellation;
- Rust async/futures/tokio, worker pools, or scheduler controls;
- resumable generators/coroutines;
- direct Rust import/export or Cargo-crate interop; or
- full C import/export syntax, header generation, or ABI lowering (V4).

Shared reference-count cycles can leak until weak links are used. Detached
tasks are outside normal process-exit joining. Abort and forced process
termination are outside automatic cleanup guarantees.


# Appendix A. Historical Shipped V3 Subset

This appendix preserves the record of what originally shipped. It is not the
normative target contract above.

## A.1 Original memory subset

The original memory pillar completed workstreams Q through T:

- `defer` became `dfr`, unused `go` was removed, option-bearing parameters and
  the `O####` diagnostic family landed;
- nominal lowering enabled `@`-guarded recursive types;
- transfer was selected implicitly: clone-safe values cloned and unique-owned
  values moved;
- borrowing used a whole-binding lexical scope stack with `#` and `!`;
- `dfr`/`edf` received ownership checks; and
- `ptr[T]`/`ptr[shared,T]`, `&value`, and `*pointer` were implemented with a
  limited dereference matrix.

That subset deliberately excluded NLL, named/returned lifetimes, partial moves,
weak references, custom finalization, and authoritative path-sensitive loan
flow. Those exclusions are precisely what this reopened plan replaces.

The checked-in historical memory packages are linked from
`book/src/800_memory/_index.md` and include the `mem_*` and `fail_mem_*`
inventories. They remain regression evidence, but positive packages must be
migrated to canonical explicit syntax in the vertical slice that changes their
contract. Removed syntax stays in focused negative fixtures.

## A.2 Original processor subset

The original processor pillar completed workstreams W through Z:

- `[>]` spawned an OS thread joined at process exit;
- `chn[T]` provided unbounded MPSC send/receive and select;
- `name[mux]: T` lowered to a mutex handle with `.lock()`/`.unlock()`; and
- `call() | async` produced an internal eventual consumed by `| await`.

Transfers at those boundaries reused the original implicit clone/move rule.
Eventuals were not public types, tasks were process-joined rather than lexically
scoped, and mutexes were parameter metadata rather than first-class owned
values. `plan/V3_PROC.md` retains the detailed history and inventory links.

## A.3 Confirmed reasons for reopening

The current implementation has four confirmed checker/backend disagreements:

1. a `[bor]` local without an initializer can silently become an ordinary
   default value;
2. a borrow local used in a later CFG block can pass `fol code check` and fail
   emitted Rust definite-initialization checking;
3. a `dfr` capture can keep a borrow alive after `!view`, allowing an owner use
   that emitted Rust rejects; and
4. the current ownership flow state omits loan state, making conditional
   giveback path-unsound.

Generated safe Rust catches several of these before native UB, but that does
not make the FOL checker sound or authoritative. Slice A fixes the model rather
than adding backend-specific workarounds.

## A.4 Historical artifacts to retain

Keep, with explicit historical labeling:

- pre-implementation parser/typecheck snapshots that explain why nominal
  lowering, parameter options, diagnostics, and processor grammar were added;
- the completed Q/R/S/T and W/X/Y/Z milestone record in repository history;
- the original positive/failure example packages as migration evidence; and
- the `core`/`memo`/bundled-`std` capability-tier distinction.

Do not retain old `Status: complete`, implicit-transfer, no-NLL, no-finalizer,
no-weak-reference, internal-eventual, or process-global-join claims as present
tense language semantics.
