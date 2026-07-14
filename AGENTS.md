# AGENTS

This is a very new project.

Only the project owner is using it.

## Project Snapshot

FOL is a new language implementation with a Rust workspace organized around:

- compiler crates
- execution/runtime crates
- tooling/editor crates
- the book as the language contract

The repo is not trying to preserve old designs for stability's sake.
If the language or build system chooses a cleaner direction, the old one should
be deleted and the codebase should move fully to the chosen model.

The practical source of truth is split like this:

- the compiler crates define meaning
- the runtime crate defines execution/runtime support contracts
- the tooling crates adapt compiler truth to CLI/LSP/tree-sitter/editor flows
- the book explains intended and current behavior

When there is tension between an old implementation and the current book/plan
direction, prefer aligning the codebase to the current chosen direction, not
preserving the historical path.

## Runtime Model Division

FOL currently has two public capability modes selected in `build.fol` through
`fol_model`, plus one bundled shipped standard-library dependency.

The capability modes are real semantic boundaries, not documentation-only
labels.

### `core`

Meaning:

- no heap
- no source-level hosted OS/runtime APIs
- executable artifacts are allowed

Expected surface:

- scalar values
- arrays
- records and entries
- ordinary routines and method sugar
- `dfr`
- optional and error shells
- array `.len(...)`

Forbidden examples:

- `str`
- `vec[...]`
- `seq[...]`
- `set[...]`
- `map[...]`
- `.echo(...)`

### `memo`

Meaning:

- heap-backed runtime facilities
- still no source-level hosted runtime APIs
- executable artifacts are allowed

Expected surface:

- everything allowed in `core`
- `str`
- `vec[...]`
- `seq[...]`
- `set[...]`
- `map[...]`
- dynamic/string `.len(...)`

Still forbidden (unless the bundled internal `standard` dependency is
declared, which upgrades the effective API tier to hosted):

- `.echo(...)`
- processor and other hosted language APIs

Practical rule:

- start with `core`
- move to `memo` only when heap-backed values are actually needed
- add bundled `std` only when shipped hosted-library wrappers are actually needed

Execution rule:

- `core` and `memo` executables may be built, run, and tested without bundled
  `std`
- bundled `std` gates source-visible hosted APIs; it does not grant permission
  to execute an artifact
- cross-target execution still requires an appropriate runner; bundled `std`
  does not make a foreign target executable on the build host
- the frontend launching a host-compatible executable, compiler, linker, or
  other system tool is build-host behavior and is orthogonal to `fol_model`
- generated recoverable entry wrappers use the shared backend-only
  `fol_runtime::process` adapter; that adapter is not re-exported as a
  `core`, `memo`, or `std` source capability

Important:

- `std` is not the informal default baseline just because the backend currently
  emits hosted Rust
- model legality must be enforced in typecheck, frontend routing, backend
  emission, editor semantics, docs, and examples

Bundled std rule:

- `std` is not a third `fol_model`
- bundled std must be declared explicitly through:
  `build.add_dep({ alias = "std", source = "internal", target = "standard" })`
- source code then reaches it through:
  `use std: pkg = {"std"};`
- bundled `std` should stay intentionally small and honest
- only actually shipped public std names should be documented as available
- internal runtime rename work such as `alloc` -> `memo` is implementation
  cleanup, not a public library contract change

Book and docs references:

- `docs/runtime-models.md`
- `docs/bundled-std.md`
- `book/src/055_build/200_graph_api.md`
- `book/src/050_tooling/350_compiler_integration.md`

## Compiler And Tooling Division

FOL is intentionally split into layers.

### Compiler truth

These crates define language meaning:

- `fol-stream`
- `fol-lexer`
- `fol-parser`
- `fol-package`
- `fol-resolver`
- `fol-typecheck`
- `fol-lower`
- `fol-intrinsics`
- `fol-diagnostics`

Rough pipeline:

- lexer turns source into tokens
- parser builds syntax/AST
- package/loading resolves package/build boundaries
- resolver binds names and imports
- typecheck enforces legality and capability rules
- lowering turns typed programs into backend-oriented IR

### Runtime and backend

These crates define executable behavior:

- `fol-runtime`
- `fol-backend`
- `fol-build`

Key current rule:

- `fol-runtime` is one crate with internal `core`, heap, and `std` modules
- `fol-backend` must emit against the correct runtime tier
- `fol-build` owns the `build.fol` graph/eval surface

### Tooling and editor

These crates adapt compiler truth to the user-facing tools:

- `fol-frontend`
- `fol-editor`

Important project rule:

- when lexer/parser/typecheck/build behavior changes, the equivalent editor
  behavior must be audited too
- the LSP should reuse compiler truth, not invent a parallel semantic model
- tree-sitter is syntax-oriented and can stay handwritten, but duplicated facts
  should be validated or generated from compiler-owned truth

Feature-completeness rule:

- a language feature is not considered complete just because the compiler
  accepts it
- every implemented feature must be checked for its editor mirror too
- some of that mirror is automatic because the LSP reuses compiler-backed
  parse/resolve/typecheck state
- but not everything is automatic
- tree-sitter grammar, query files, highlighting, locals, symbols, build-file
  editor behavior, some completion surfaces, and some LSP UX behavior still
  require explicit audit and sometimes explicit updates
- if a feature changes syntax, declarations, names, scopes, intrinsics,
  build-surface behavior, or model availability, the editor/tree-sitter side
  must be reviewed in the same change set
- if no editor change is needed, that should be because the feature was
  verified to already flow through existing compiler-backed editor paths, not
  because the editor was ignored

That means new work often has two halves:

- compiler/runtime implementation
- editor/tree-sitter/LSP synchronization

Book references:

- `book/src/050_tooling/350_compiler_integration.md`
- `book/src/050_tooling/400_treesitter.md`
- `book/src/050_tooling/500_lsp.md`
- `book/src/050_tooling/450_feature_checklist.md`

Current editor non-goals:

- no broad V2-aware editor behavior beyond the checked-in shipped generic
  routine and protocol-standard example subset
- no range formatting
- no broad rename beyond the current safe supported scope
- no editor-owned semantic rules that compete with compiler-backed analysis
- no documentation claiming wider code-action coverage than the shipped exact
  replacement flow

## Bundled Std Rule

`std` is bundled with FOL under `lang/library/std`.

Important:

- only `std` is importable from source code
- `core` and `memo` remain capability modes, not libraries
- bundled `std` is reached through the explicit internal dependency:
  `build.add_dep({ alias = "std", source = "internal", target = "standard" })`
- bundled `std` should grow mostly in FOL source
- low-level hosted/runtime substrate can remain Rust-backed

When changing bundled `std`:

- keep the public surface honest and intentionally small
- source imports must use quoted targets only, for example:
  `use std: pkg = {"std"};`
- do not reintroduce old unquoted import-target forms
- do not reintroduce the removed `std` source kind in docs, tests, or examples
- add or update a real example package for new public names
- add or update integration coverage
- add or update LSP/tree-sitter coverage
- update bundled std docs and the book in the same change
- do not document future std modules as if they already ship
- keep one exact shipped-surface matrix in sync:
  - real module files
  - real public routine names
  - real canonical example packages
- keep raw substrate examples explicit and minimal
- do not let ordinary hosted examples drift back to direct `.echo(...)` if an
  equivalent bundled `std.io` wrapper exists
- if you rename the internal runtime seam, update emitted-runtime tests and
  backend trace expectations in the same change

## Version Targets

The book already separates current language contract from later design work.
Work should stay conscious of that split.

### `V1`

This is the current implemented language milestone.

Examples of current V1 material:

- routines and current call binding
- method sugar for records
- recoverable errors
- narrow `dfr`
- current runtime-model split
- current build system surface

Use the book's explicit current-boundary chapters when deciding whether
something belongs in V1.

References:

- `book/src/500_items/200_routines/_index.md`
- `book/src/650_errors/200_recover.md`
- `book/src/700_sugar/250_defer.md`
- `book/src/300_meta/100_buildin.md`

### `V2`

This is later language expressiveness and contract work, not current compiler
surface.

Examples already marked as V2-oriented in the book:

- standards
- blueprints/extensions
- generics
- broader contract-style language surfaces

Current landed `V2` subset:

- Milestone 1
  - executable generic routines
  - narrow argument-driven inference only
  - generic types exist in the shipped full-`V2` contract, but they are not
    part of the narrow Milestone 1 core itself
  - generic routine lowering/backend execution now exist for the shipped
    positive Milestone 1 examples
  - deeper example-driven hover/definition and tree-sitter audit now exist for
    the checked-in Milestone 1 examples
- Milestone 2
  - protocol standards only
  - required receiver-qualified routines only
  - explicit type-side conformance headers
  - conformance checking in typecheck
  - no blueprint or extended semantic support
  - protocol standards now lower and the shipped positive protocol examples
    execute through ordinary procedural emission
  - deeper example-driven hover/definition and tree-sitter audit now exist for
    the checked-in Milestone 2 examples

When changing `V2` surfaces:

- keep milestone docs honest
- keep explicit lowering boundaries honest when later stages still do not exist
- update examples and negative examples in the same change
- audit `fol-editor` and tree-sitter in the same change set

References:

- `book/src/500_items/400_standards.md`
- `book/src/500_items/500_generics.md`

Current landed subset note:

- the active `V2` Milestone 1 work currently covers only a narrow
  generic-routine core
- parser, resolver, typecheck, lowering, backend, and editor coverage are now
  real for the shipped positive Milestone 1 examples
- broader generic work beyond that narrow core should stay explicit in docs,
  tests, and examples rather than being inferred from the milestone name
- the active `V2` Milestone 2 work currently covers only protocol standards and
  narrow procedural conformance
- when hardening `V2`, keep the checked-in positive and negative example matrix
  synchronized with milestone docs, editor coverage, and current execution
  expectations
- if V2 editor or tree-sitter coverage is expanded, keep semantic tokens,
  locals, symbols, and example inventory checks aligned in the same change

### `V3`

This is the current shipped systems/runtime milestone. It has two complete
pillars:

- memory: ownership, lexical borrowing, typed unique/shared pointers, and
  ownership-aware `dfr` / `edf`
- processor: OS-thread spawn, channels, `select`, `[mux]`, and internal
  eventuals through `| async` / `| await`

Current deliberate V3 boundaries include:

- dereferencing a unique pointer reached through a record field is rejected
  until lowering has place-aware projection IR
- a moved owner cannot be reinitialized inside `dfr` or `edf`, because the
  delayed assignment has not happened when the deferred body is registered
- spawn and async task calls must target direct named routine declarations;
  stored routine values and routine parameters are not indirect task targets
- deferred bodies cannot access mutex fields, call `.lock()` / `.unlock()`, or
  forward a mutex handle to another `[mux]` routine

V3 work is incomplete unless the whole shipped surface is synchronized across
compiler semantics, lowering, runtime/backend, frontend routing, diagnostics,
LSP, tree-sitter grammar/queries/corpus, examples, tests, docs, and the book.
The editor must reuse compiler truth for semantic behavior, but its explicit
syntax, completion, token, inventory, and UX mirrors still have to be audited.

References:

- `book/src/800_memory/100_ownership.md`
- `book/src/800_memory/200_pointers.md`
- `book/src/900_processor/100_eventuals.md`
- `book/src/900_processor/200_corutines.md`
- `book/src/700_sugar/250_dfr.md`

### `V4`

This is later interop/backend boundary work.

Examples already marked as V4-oriented in the book:

- C ABI
- Rust interop
- related casting/diagnostic/backend package work

References:

- `book/src/750_conversion/200_casting.md`
- `book/src/650_errors/300_diagnostics.md`
- `book/src/500_items/300_constructs/100_aliases.md`

When implementing or reviewing a feature:

- confirm whether the book presents it as current V1 or later V2/V3/V4 design
- if it is later-version material, do not silently implement it as part of V1
- if V1 is chosen explicitly, update code, tests, docs, editor, and examples
  together

## Legacy Policy

Do not preserve legacy behavior just because it existed first.

When a new feature, API, syntax, or system replaces an old one:

- remove the old one
- do not keep compatibility shims
- do not keep fallback paths
- do not keep parallel implementations
- do not add migration helpers
- do not add migration warnings
- do not add deprecation period

If the new way is chosen, the old way should be deleted.

## Build-System Policy

For the build system specifically:

- no legacy `def root: loc = ...`
- no legacy `def build(...)`
- no compatibility-root behavior
- no hybrid routing
- no compatibility parsing path

The codebase should move directly to the current build model and delete the old path.


## COMMITING TO GIT
 IN NOW WAY OR FORM YOU ARE ALLOWD TO PUT SIGNATURE IN GIT COMMITS
 GIT COMMITS SHOULD BE TITLE ONLY, AND FOLLOW CONVENTIONAL COMMITS

 <commit_type>(<if_nneded_scope>):<message_max_50_characters>
