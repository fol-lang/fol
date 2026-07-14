# FOL Version Boundaries

Last updated: 2026-07-14

This file explains how the language should be grouped into `V1`, `V2`, `V3`,
and `V4`.

It is not a task list.
It is not a parser checklist.
It is not a promise that every chapter already works.

Its purpose is to keep one distinction clear while the compiler grows:

- the book describes the intended language
- the parser may accept a large surface of that language
- the released compiler versions should promise only the semantic subset that is
  actually implemented end to end

## The main rule

FOL already has broad syntax coverage. That is good, but it is not the same as
 saying a feature is implemented.

For versioning purposes, a feature is only considered part of a version when
the repository supports it through the full chain that matters for that
feature.

That usually means:

- the syntax is parsed
- names can be resolved
- the relevant semantic phase enforces the feature correctly
- diagnostics are explicit when the feature is used incorrectly
- the later compiler stages needed by that feature are present too

For a shipped feature with runtime or editor impact, that chain also includes
the applicable runtime/backend behavior, frontend routing, formatter and tool
commands, LSP behavior, tree-sitter grammar/queries/corpus, positive and
negative examples, machine inventories, tests, docs, and book text. A layer may
need no code change, but it must be audited rather than silently omitted.

So:

- parsed is not the same thing as implemented
- resolved is not the same thing as implemented
- a future-facing chapter in the book is not automatically a `V1` commitment

This matters because FOL should be honest about what each release guarantees.

## Runtime model note

The language version and the runtime capability model are different axes.

FOL uses a build-selected runtime model:

- `core`
  no heap, no source-level hosted OS APIs
- `memo`
  adds alloc-like heap-backed facilities, still no source-level hosted OS APIs

Those are the only public `fol_model` values. Bundled `std` is a shipped
internal dependency, not a third model; `std` is not accepted as a
`fol_model` value.
The build graph declares the hosted library explicitly:

```fol
build.add_dep({ alias = "std", source = "internal", target = "standard" });
```

Source then imports the dependency with `use std: pkg = {"std"};`. The
compiler derives its effective capability tier from both inputs:

| Declared `fol_model` | Bundled `std` dependency | Effective tier |
| --- | --- | --- |
| `core` | absent | core |
| `memo` | absent | memo |
| `memo` | present | hosted std |

Bundled `std` requires `memo`; it does not make a `core` artifact heap-capable.

This model should be selected per build artifact through `build.fol`, not by a
source-file pragma.

The build entry surface itself is also ordinary FOL:

- canonical entry is `pro[] build(): non`
- package metadata and direct dependencies are configured through `.build()`
- graph access is reached through `.build().graph()`
- neither the build handle nor the graph handle is a public type name in source code

That means a `V1` compiler can still have multiple runtime models. The version
answers “which language semantics are implemented end to end,” while the model
answers “which runtime capabilities this artifact is allowed to use.”

For the runtime split:

- arrays, scalars, records, routines, control flow, `dfr`, and
  `opt[...]`/`err[...]` belong to `core`
- heap-backed `str`, `vec`, `seq`, `set`, and `map` belong to `memo`
- hosted wrappers are reached through explicit bundled `std`

Current contract:

- `core` means no heap and no source-level hosted OS/runtime APIs
- `memo` means alloc-like heap-backed runtime facilities without source-level
  hosted process/OS APIs
- bundled `std` remains an explicit shipped library dependency on top of
  `memo`
- `core` and `memo` artifacts may both build, run, and test without bundled
  `std`; the dependency gates hosted language APIs, not executability
- `graph.add_run(...)` and `graph.add_test(...)` do not grant or require
  bundled-`std` capabilities; they only request host execution of a selected
  artifact
- host-compatible artifact and system-tool launching is frontend/build-host
  behavior, separate from the language capability model
- cross-target artifacts require an external runner; the current `run` / `test`
  commands are host-only and reject those artifacts
- a recoverable entry routine is adapted to process exit by the backend-only,
  capability-neutral `fol_runtime::process` seam; that adapter is not a public
  source API and does not upgrade the artifact to hosted std

Current implementation honesty note:

- `core` is already enforced as a language/runtime capability boundary
- `core` still goes through the current Rust backend pipeline today
- so `core` should be read as “no heap, no source-level hosted OS/runtime APIs”
  rather than as “the whole generated binary performs no allocation” or
  “embedded backend complete”

This split is a runtime capability boundary, not an object-model feature and
not a source-file pragma.

## How to read the book through versions

The book is a language-design document. It covers:

- core language syntax
- type families
- declarations
- contracts
- memory model
- concurrency
- module layout
- error handling
- sugar
- conversion

Those chapters do not all belong to the same release milestone.

Some chapters describe core language that should work in the first usable
compiler.
Some chapters describe richer semantic systems that depend on type checking and
conformance machinery.
Some chapters describe systems behavior that depends on ownership, concurrency,
foreign interfaces, packaging, linking, and backend work.

That is why the language should be grouped into four semantic releases instead
of trying to make the whole book real at once.

## What V1 means

`V1` is the first compiler release that takes ordinary FOL source through
package loading, resolution, type checking, lowering, and native backend
emission for a useful non-interop subset of the language.

The key idea is coherence.
`V1` does not need every ambitious feature from the book.
`V1` needs one subset that is strong enough to be real, teachable, testable,
lowerable, and buildable from source to binary.

At the current repository head, that already means more than front-end validity.
The implemented `V1` compiler chain now reaches:

- `fol-stream`
- `fol-lexer`
- `fol-parser`
- `fol-package`
- `fol-resolver`
- `fol-typecheck`
- `fol-lower`
- `fol-runtime`
- `fol-backend`
- `fol-frontend`

The bounded V1 subset therefore reaches a produced binary. Later language
surfaces remain separate version work; they are not a missing first-backend
stage.

### V1 is the core language

Based on the current book, `V1` should cover the parts of FOL that behave like
the essential language core:

- lexical structure
- names and package layout
- imports and package visibility
- ordinary declarations and bindings
- functions and procedures
- ordinary control flow
- literals and basic expressions
- aliases
- records and entries
- field access and ordinary initialization
- builtin scalar types
- a practical subset of containers
- `panic` and `report`
- enough conversion and coercion rules to type-check normal code

These are the chapters and surfaces that naturally fit there:

- `100_lexical/*`
- the core parts of `200_expressions/*`
- the core parts of `400_type/*`
- `500_items/100_variables.md`
- `500_items/200_routines/*`
- `500_items/300_constructs/100_aliases.md`
- `500_items/300_constructs/200_structs.md`
- `600_modules/*`
- `650_errors/*`
- the sugar chapters whose behavior lowers cleanly into already-supported core semantics

### Why aliases belong in V1

The alias chapter is not an advanced research feature.
It is a normal way to name types, simplify signatures, and attach methods to a
named type surface.

That means aliases are part of the core language, not a later experimental
layer.

So `ali` belongs in `V1`.

### Why records and entries belong in V1

Records and entries are also core language material.

They are not just syntax niceties.
They are the ordinary way to model structured data.
Without them, the language would be missing a basic user-defined type story.

So records and entries belong in `V1`.

### What V1 should not promise

`V1` should not pretend to support features that require major semantic systems
the compiler does not yet have.

That means a `V1` compiler should reject such features explicitly instead of
letting them pass because the parser happened to accept the syntax.

## What V2 means

`V2` should be the advanced language-semantics release.

This is the point where FOL stops being only a core language and grows its more
ambitious abstraction systems.

The important thing about `V2` is that it is still primarily about language
semantics, not low-level systems interop.

### V2 is where contracts and advanced abstraction belong

Current implemented `V2` subset at repo head:

- Milestone 1
  - executable generic routine core
  - narrow argument-driven inference only
  - parser, resolver, typecheck, lowering, and backend execution for the
    checked-in positive subset
  - expanded positive and negative example coverage
  - deeper example-driven editor and tree-sitter coverage
- Milestone 2
  - protocol standards only
  - required receiver-qualified routine signatures only
  - explicit type-side conformance headers
  - conformance checking in typecheck
  - procedural lowering and backend execution for the checked-in positive
    protocol subset
  - expanded positive and negative example coverage
  - deeper example-driven editor and tree-sitter coverage
- Combined shipped V2 surface
  - executable generic types for the checked-in concrete M1/M2 examples
  - generic/protocol-standard constraints only where the shipped example
    matrix exercises them

Still future in `V2`:

- generic constraints beyond the landed narrow subset
- blueprint standards as semantic contracts
- extended standards as semantic contracts
- standards-based dispatch/inference
- broader contract machinery

The book chapters that clearly fit here are:

- `500_items/400_standards.md`
- `500_items/500_generics.md`
- much of the metaprogramming surface in `300_meta/*`
- the more advanced sugar/type interactions that need richer semantic analysis

This is where the language starts needing machinery such as:

- generic parameter checking
- constraint checking
- conformance checking
- method-set fulfillment
- contract satisfaction
- richer dispatch rules
- more advanced conversion and inference behavior

### Why standards and blueprints belong in V2

The standards chapter is not just syntax.
It describes semantic enforcement.

A standard is meaningful only if the compiler can answer questions like:

- does this type fulfill the required methods?
- does this type fulfill the required data members?
- may this value be used where the standard is expected?
- do extension surfaces and protocol/blueprint contracts actually hold?

That is already beyond basic parsing and ordinary name resolution.
It depends on a real semantic conformance system.

That means:

- protocols belong in `V2`
- blueprints belong in `V2`
- extensions in the contract sense belong in `V2`

So if a `V1` compiler sees these surfaces, it should say they are not
implemented yet rather than silently pretending the syntax alone is enough.

### Why generics belong in V2

The generics chapter is also beyond the core language.

Generic syntax by itself is not the hard part.
The hard part is semantic behavior:

- parameter binding
- specialization rules
- checking constraints
- dispatch interactions
- generic type construction
- generic method calls

That is too much to bundle into the first core-language milestone.

So generics belong in `V2`.

### Other features that naturally fit V2

Several other book surfaces look small on the page but actually imply deeper
semantic machinery.

Those should also be treated as `V2` unless later implementation proves they
really belong elsewhere:

- `any` and `union`-style semantics from `400_type/400_special.md`
- advanced method dispatch
- advanced matching and rolling behavior when it depends on richer typing
- logic-flavored data and query surfaces such as `axi` and some logical routine
  use cases
- meta-level language features that need compile-time semantic reasoning rather
  than just syntax preservation

These are still language features, but they are not the first batch.

## What V3 means

`V3` is the shipped systems-semantics release.

The compiler now includes the deeper resource and runtime semantics that the
language design points toward, rather than stopping at ordinary typed-language
semantics.

### V3 owns memory and concurrency

The current contract chapters are:

- `800_memory/100_ownership.md`
- `800_memory/200_pointers.md`
- `900_processor/100_eventuals.md`
- `900_processor/200_corutines.md`

### Why ownership belongs in V3

Ownership is not just another type rule.
It is a resource and lifetime system.

The shipped ownership and borrowing contract answers these questions:

- when values move
- when values are invalidated
- when borrowing is legal
- when mutable borrowing is exclusive
- when pointers and ownership interact
- when destruction is safe

That deep semantic layer is the V3 memory pillar rather than an extension of
the first type-checking and lowering milestone.

### Why concurrency belongs in V3

Eventuals, spawn, channels, mutex-style routine passing, and task semantics
require more than expression typing.

They require:

- runtime model decisions
- scheduling or execution model assumptions
- channel/message typing
- synchronization semantics
- error and cancellation behavior
- explicit lowering and runtime contracts

The shipped processor pillar makes those choices explicitly: one OS thread per
task, join-at-exit, unbounded MPSC channels, source-order `select`, `[mux]`
shared mutation, and internal eventuals. This is separate from both V1 and the
standards/generics work in V2.

### The V3 pillar split and its detailed plans

`V3` was implemented as two ordered pillars, each with its own detailed plan.
Those plans remain the implementation record. The linked memory and processor
book chapters and this file define the current user-facing contract.

- the memory pillar — `plan/V3_MEM.md`
- the processor pillar — `plan/V3_PROC.md`

The memory pillar completed fully before the processor pillar began.

Memory pillar (`plan/V3_MEM.md`):

- shared prep: rename the shipped keyword `defer` to `dfr` everywhere, remove the
  unused reserved keyword `go`, fix the sigil charter, delete the
  `ALL_CAPS`-means-borrowable convention, and add an `O####` OWNERSHIP
  diagnostic family
- Milestone 1: ownership with the initial static move/clone cases (plain stack
  values clone and `@` heap values move), plus the flagship recursive heap
  types (`opt @Node`) through a new nominal lowered-type representation; later
  memory milestones generalize transfer by recursive type ownership
- Milestone 2: scope-granular borrowing (`var[bor]`, `#x`, `!x`, `name[bor]:`
  parameters), ownership-aware `dfr`, and error-only `edf`
- Milestone 3: typed pointers `ptr[T]` (unique) and `ptr[shared, T]`
  (refcounted), `&x` address-of, `*p` deref; raw pointers stay a `V4` interop
  boundary
- ownership/borrow checking is compile-time and legal in every runtime model;
  anything that allocates requires `memo` or higher

Processor pillar (`plan/V3_PROC.md`):

- the entire processor surface is `std`-only; `core` and `memo` reject it with
  tier diagnostics
- concurrency is OS threads through the Rust standard library, with no async
  runtime, no worker pool, and no colored functions
- P1: fire-and-forget `[>]` spawn with thread-per-spawn execution and
  join-all-at-exit; awaitable work uses `call() | async` instead
- P2: `chn[T]` unbounded MPSC channels with pipe send and a blocking pull
  receive
- P3: multi-arm `select` multiplexing and `name[mux]:` mutex parameters
- P4: eventuals through `| async` and `| await`, with an internal (not
  user-nameable) eventual type, must-handle recoverable obligations, and error
  handling identical to the synchronous call site
- an eventual can be awaited at most once; a recoverable eventual must be
  awaited and handled before fallthrough or an exiting `break`, `return`, or
  `report`, while an infallible eventual may remain for the process-exit join
- nested routines cannot implicitly capture outer locals, and `edf` cannot
  await or access an existing eventual binding

Both V3 pillars are implemented across the compiler, runtime/backend, frontend
artifact routing, diagnostics, formatter/tool commands, LSP, tree-sitter,
examples, tests, docs, and book. Processor P1 uses one OS thread per spawn and
joins at exit; P2 ships unbounded MPSC channels; P3 ships source-order polling
select and `[mux]` shared mutation; P4 ships internal eventuals with synchronous
error transparency and path-checked recoverable obligations. Every processor
surface remains `std`-only.

That implementation claim includes the explicit mirrors, not only compiler
acceptance: evaluated artifact capabilities, diagnostics and explanations,
formatter and tool-command behavior, LSP behavior, tree-sitter grammar and
queries, corpus fixtures, positive and negative example inventories, docs, and
book chapters must stay synchronized whenever a V3 boundary changes.

Generator semantics are not part of either V3 pillar. The language keyword
`yield` is retained by the lexer, parser, resolver, and syntax-oriented editor
assets, but the current typechecker rejects it and lowering keeps a defensive
unsupported boundary. That syntax preservation is not a shipped generator
contract, and generators remain later design with no current version promise.

## What V4 means

`V4` should be the interop and toolchain-boundary release.

This is where the compiler becomes a deliberate participant in foreign
toolchains rather than only a native Rust-emitting compiler.

### V4 is where foreign interop and ABI work belong

The strongest candidates already visible in the repository direction are:

- C ABI support
- Rust interop
- header import/export
- native objects and libraries
- linker-facing build metadata

### Why C ABI belongs in V4

Foreign interop crosses several compiler layers at once.

It is not only a typechecker task.
It needs:

- package/build ownership of native artifacts
- foreign declaration modeling
- ABI-safe type checking
- symbol import/export handling
- later linker/backend integration

That is why C ABI should be a `V4` feature, not something forced into the early
language milestones.

### Why Rust interop belongs in V4

Rust interop is not just "emit Rust" in reverse.

It needs:

- foreign symbol and type modeling
- backend/linker coordination with external Rust crates
- stable lowering rules for imported Rust functions and types
- a clear boundary between FOL semantics and Rust-specific ownership/ABI details

That makes Rust interop later than core `V3` systems semantics. It belongs in
`V4` together with C ABI and the rest of the foreign-toolchain boundary work.

## The boundary between syntax and promise

One of the most important consequences of this version split is how the compiler
should behave before a version is complete.

If a feature belongs to a later version, the compiler should prefer this shape:

- parse it if the syntax is already supported
- preserve enough structure for future work if useful
- reject it explicitly at the semantic phase that would otherwise imply support

In other words:

- syntax may arrive earlier than semantic implementation
- but release promises must follow semantic ownership, not parser coverage

That keeps the language honest and lets the parser stay broad without forcing
the compiler to fake support for every surface immediately.

## How sugar should be classified

The sugar chapters should not be versioned independently from the semantics they
lower into.

The right rule is:

- a sugar feature belongs to the earliest version whose underlying semantics
  fully exist

That means:

- if a sugar form is only a nicer spelling of core `V1` behavior, it belongs in `V1`
- if it depends on richer typing, matching, contracts, or inference, it likely
  belongs in `V2`
- if it depends on ownership, concurrency, or runtime systems behavior, it
  belongs in `V3`
- if it depends on foreign ABI, Rust interop, or linker/build coordination, it
  belongs in `V4`

So sugar does not get a free pass just because it looks syntactically small.

One useful example is `dfr`:

- a narrow lexical-scope `dfr { ... }` that only guarantees scope-exit
  execution order is compatible with `V1`
- a more complicated `dfr` model that depends on ownership, borrowing,
  pointer/resource cleanup, async task cleanup, or foreign/native resource
  coordination belongs later
- ownership-aware and runtime-aware `dfr` semantics therefore belong with the
  `V3`/`V4` milestones that own those semantics

## Release sequence and current position

The semantic milestone order remains:

- `V1`: the binary-producing core language
- `V2`: advanced language semantics and the shipped narrow generic/protocol
  subset
- `V3`: shipped systems semantics
- `V4`: future interop and ABI work

At the current repository head, `V1`, the explicitly bounded `V2` subset, and
both `V3` pillars are implemented end to end. Broader V2 contract machinery and
V4 interop remain future work; generators and language `yield` also remain
future work outside the current V3 contract. None of those later surfaces is
evidence that V3 is still pending.

The practical rule is therefore:

- keep every shipped V1/V2/V3 surface synchronized through compiler, runtime,
  backend, frontend routing, diagnostics/explanations, formatter and tool
  commands, LSP, tree-sitter grammar/queries/corpus, examples, tests, docs, and
  book
- reject only the unimplemented parts of the broader design with explicit,
  owner-appropriate diagnostics
- do not claim the whole book merely because its syntax parses

## Summary split

If the language needs only core typing and normal program semantics, it is
probably `V1`.

If the language needs conformance, generics, richer compile-time abstraction, or
advanced type semantics, it is probably `V2`.

If the language needs ownership, borrowing, pointers, concurrency/runtime
coordination, or execution-model semantics, it is probably `V3`.

If the language needs foreign interop, C ABI, Rust interop, native library
linking, or build/linker cooperation, it is probably `V4`.

That is the line this repository should keep using while the compiler grows.
