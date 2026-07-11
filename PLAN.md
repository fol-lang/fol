# V2 Deepening Plan

The previous V2 Continuation plan is complete. Its workstreams A through F
are closed.

This plan covers the next round of V2 work. The theme is:

- no OO, no classes, no inheritance, no runtime dispatch, no vtables
- lean heavily on compile time
- take advantage of the Rust lowering target: anything Rust already does at
  compile time through monomorphization is essentially free to expose in FOL
- the parse surface must not carry features we will never build
- every new feature must monomorphize cleanly
- every feature change must be mirrored in the LSP and in the tree-sitter
  grammar in the same change set, not later


# 1. Guardrails

Keep, permanently:

- generic routines
- generic types
- protocol standards
- standards-as-constraints
- static conformance checking
- monomorphized lowering and backend emission
- procedural call binding
- no hidden runtime dispatch layer

Never add, under any workstream in this plan:

- object-style dispatch
- `dyn` / vtable / witness-table systems
- standards used as value or parameter types
- inheritance or base classes
- global solver-style inference
- first-class generic routine values via type erasure
- implicit runtime dispatch behind any surface
- anything that forces boxing or heap indirection for polymorphism

If any workstream below drifts toward those, that workstream stops.


# 2. Current Truth Snapshot

Shipped and honest narrow V2:

- generic routines with argument-driven inference
- generic types with explicit instantiation
- generic receiver routines with FOL-side monomorphization and `self`
  receiver access
- protocol standards with procedural conformance
- standards-as-constraints on generic routines

Parsed but explicitly rejected today (the current "deferred" surface):

- blueprint standards `std X: blu`
- extended standards `std X: ext`
- standards used as ordinary value or parameter types
- `imp Self: std = { ... }` implementation blocks
- receiver-qualified required routines inside a `std` body
- capturing required routines inside a `std` body
- generic required routines inside a `std` body
- default-body standard routines
- generic error types on routines
- explicit generic-call syntax

This plan decides what each of those becomes: built, rebuilt, or deleted.


# 3. Exit Criteria

This plan is complete when all of the following are true:

1. Every feature in the "additions" workstreams (H through O) is implemented
   end to end: parser, resolver, typecheck, lowering, backend, LSP,
   tree-sitter, positive and negative examples, and docs.
2. Every feature in the "removals" workstream (G) is gone from the grammar,
   the AST, the resolver, the typecheck rejections, the tree-sitter grammar,
   the LSP queries, the docs, and the examples.
3. There is no "planned for a future release" rejection message anywhere in
   the compiler that refers to a feature this project has already chosen not
   to build.
4. No OO-shaped syntax remains in the parser.
5. The V2 chapters in the book describe the resulting language exactly, with
   no leftover pre-removal wording.


# 4. Workstream G: Remove Surfaces We Will Not Build

This workstream must land first. The grammar should stop describing a
language we are not building. Every removal here is a real delete: not a
new "deferred" comment, not a new TODO, not a new "planned for a future
release" branch.

## G1. Remove `imp Self: std = { ... }` implementation blocks

Why remove:

- `imp Self: std = { ... }` is the classic shape of a type getting methods
  dispatched through a shared interface — that is an OO / trait-object
  pattern
- FOL has already chosen that standards are static contracts expressed
  through receiver routines on the conforming type itself, and nothing more

Work:

- parser: remove the `imp` declaration production and its AST node
- resolver: remove `imp` symbol binding
- typecheck: delete the "implementation declarations are planned for a
  future release" rejection — the syntax no longer exists
- tree-sitter grammar: remove any `imp_decl` / `implementation_block` rule
- tree-sitter queries: remove every reference to removed nodes
- LSP: drop any symbol, hover, or navigation path keyed on `imp`
- tests: drop `standards_m2_reject_implementation_dispatch_surfaces_cleanly`
  and any fail example keyed on `imp`
- docs: remove `imp` from the book and from every `docs/v2-*` design doc
- examples: delete any example file that uses `imp`

## G2. Make "standards in type position" a permanent error

Why:

- a standard in value or parameter position would require boxing or
  `dyn Trait`
- that is against the guardrails permanently, not temporarily

Work:

- parser: no change (standard identifiers in type position are parsed as
  ordinary type references — that is fine)
- typecheck: keep the rejection that fires when a standard symbol appears
  in type position, but replace the message
  "standard 'X' cannot be used as an ordinary type in V2 Milestone 2"
  with a permanent message such as
  "standards are static contracts, not value types; use them as generic
  constraints instead"
- tests: update the existing assertions to the new message text
- docs: remove any prose that implies this could later become possible
- examples: the existing `fail_standard_as_type_m2` example keeps its
  intent; update expected diagnostics

## G3. Remove receiver-qualified required routines inside `std` bodies

Why:

- required routines are already mandatory receiver routines on the
  conforming type
- re-declaring the receiver inside the standard body is redundant and
  adds no expressive power
- it is also misleading: it reads like the standard is "dispatching" to a
  receiver, which is not what happens

Work:

- parser: remove the receiver slot from the `standard_requirement` grammar
- AST: remove the receiver slot from the required-routine node
- resolver: drop receiver handling inside standards
- typecheck: delete the "receiver-qualified standard requirements are not
  yet supported in V2 Milestone 2" rejection — the shape no longer exists
- tree-sitter grammar: remove the receiver slot inside
  `standard_requirement`
- tree-sitter queries: audit highlights / locals / symbols for removed
  references
- LSP: audit hover and definition inside standard bodies so that no code
  path assumes a receiver
- tests: drop the corresponding pinning test
- docs: remove the receiver form from the standards chapter

## G4. Remove capturing required routines inside `std` bodies

Why:

- capture state is an implementation detail of the conforming routine
- a static contract should not dictate whether a conformer closes over
  state

Work:

- parser: remove the capture slot from `standard_requirement`
- AST: remove the capture slot from the required-routine node
- typecheck: delete the corresponding "not yet supported" rejection
- tree-sitter grammar: remove the capture slot inside
  `standard_requirement`
- tree-sitter queries: audit
- LSP: audit
- tests: drop the corresponding pinning test
- docs: remove the capture form from the standards chapter

## G5. Remove generic required routines inside `std` bodies

Why:

- the right way to have generic behavior inside a standard is to make the
  **standard itself** generic (see Workstream O)
- per-routine generics inside a non-generic standard would require
  per-call-site conformance re-check, which drifts toward solver behavior
  and complicates diagnostics

Work:

- parser: remove the generic parameter slot from `standard_requirement`
- AST: remove the generic parameter slot from the required-routine node
- typecheck: delete the corresponding "not yet supported" rejection
- tree-sitter grammar: remove the generic parameter slot inside
  `standard_requirement`
- tree-sitter queries: audit
- LSP: audit
- tests: drop the corresponding pinning test
- docs: remove the per-routine generic form from the standards chapter,
  and explicitly point at generic standards (O) as the chosen mechanism

Acceptance for G1–G5:

- no grammar rule, no AST node, no resolver symbol, no typecheck rejection,
  no tree-sitter rule, no LSP query, no book prose, and no example carries
  the removed shapes
- the full integration test suite passes
- nothing in the book or in `docs/v2-*.md` describes these removed shapes
  as "future work"; they are gone, not delayed

Tracked slices:

- [x] G1. Remove `imp Self: std = { ... }` implementation blocks.
- [x] G2. Turn standards-in-type-position into a permanent error.
- [x] G3. Remove receiver-qualified required routines from `std` bodies.
- [x] G4. Remove capturing required routines from `std` bodies.
- [x] G5. Remove per-routine generic required routines from `std` bodies.


# 5. Workstream H: Generic Receiver Types

Goal: generic receiver routines become real, executable, typed
routines. Chosen shipped surface: the routine declares its generic
parameters explicitly — `fun (Box[T])get(T)(): T = { return self.value; }`
— keeping generic introduction visible at the declaration instead of
inferring it from the receiver slot.

Why it fits:

- Rust has this natively via
  `impl<T> Box<T> { fn get(&self) -> T { ... } }`
- monomorphizes per instantiation
- no new dispatch, no new runtime behavior
- without it, generic types are half-done: FOL can declare `Box(T)` but
  cannot sugar receiver routines onto it

Work:

- parser: already parses `fun (Box[T])...` — verify, including nested
  receiver such as `fun (Pair[K, V])first(): K`
- resolver: bind `T` in the receiver type into the routine's generic scope;
  allow argument-driven inference to flow through a `Box[int]` argument
- typecheck: delete the rejection at `decls.rs:587`; extend generic
  inference so that `box.get()` on `Box[int]` binds `T = int` without
  explicit instantiation; keep inference strictly argument-driven
- lowering: emit one Rust `impl<T> Box<T> { fn get(&self) -> T { ... } }`
  block per generic-receiver routine
- backend: verify monomorphization is driven by the same path as generic
  routines

Editor:

- LSP hover on `box.get()` shows the monomorphized signature
- LSP go-to-definition resolves to the generic receiver routine
- LSP diagnostics cite the receiver type when inference fails
- tree-sitter: confirm the grammar already allows generic types inside the
  receiver slot; add highlight or locals rules if the shape is not fully
  covered
- tree-sitter tests: parse a fixture with a generic receiver routine and
  assert the node shape

Tests:

- positive: `examples/generic_receiver_v2plus` (new)
- positive: cross-file variant
- negative: receiver uses a generic parameter that does not appear in any
  argument (inference failure)
- negative: receiver type is not declared as a generic type

Docs:

- `book/src/500_items/500_generics.md`: promote generic receiver routines
  from deferred to current contract
- every `docs/v2-*.md` that lists this as deferred must be updated

Tracked slices:

- [x] H1. Parser/AST/resolver verification and typecheck lift.
- [x] H2. Method resolution unifies generic receiver templates against
  the call-site object type and records the chosen routine symbol so
  lowering can look it up directly.
- [x] H3. Lowering / backend emission for generic receiver routines.
  Chosen strategy: FOL-side monomorphization. A post-body lowering pass
  (`fol-lower/src/mono.rs`) rewrites every call to a generic-receiver
  template into a call to a synthesized concrete clone (types substituted
  from the call-site argument types), then removes the templates, so the
  backend only sees ordinary concrete routines. Includes `self` receiver
  binding through resolver/typecheck/lowering, and a typecheck two-phase
  declaration pass so cross-file generic type templates no longer depend
  on source-unit ordering.
- [x] H4. LSP coverage. `generic_receiver_m1` and
  `generic_receiver_cross_file_m1` are in the LSP open/document-symbols
  example lists; editor diagnostics flow through compiler-backed
  typecheck.
- [x] H5. Tree-sitter grammar / queries audit. Routine declarations
  gained the `generics` slot (plain and method), `self` participates in
  field access, and the grammar was parse-validated with the tree-sitter
  CLI against every checked-in `.fol` source (examples, build files,
  bundled std, showcases) with zero ERROR nodes. Corpus files rewritten
  in real, compiler-verified syntax; highlight/locals/symbols queries now
  compile against the generated parser.
- [x] H6. Positive examples (`examples/generic_receiver_m1`,
  `examples/generic_receiver_cross_file_m1`), negative example
  (`examples/fail_generic_receiver_m1`, underconstrained routine
  generic), docs updated in `docs/v2-generics-m1.md`,
  `book/src/500_items/500_generics.md`, and the methods chapter now
  documents `self`.


# 6. Workstream I: Explicit Generic-Call Syntax

Goal: `pick[int](value)` works as an explicit instantiation escape hatch.

Why it fits:

- Rust has turbofish `pick::<i32>(value)`
- 100% static, zero runtime behavior
- strictly stronger than the current model: you never *have* to use it,
  but you can use it when argument-driven inference cannot pick a type
- unblocks future constrained generic types (Workstream K) where the
  caller sometimes needs to pin the type argument explicitly

Work:

- parser: add an explicit `name[TypeArgs](args)` call form in the
  expression grammar; disambiguate from subscript / indexing at the
  grammar level, not at semantic level
- AST: new call node carrying optional generic arguments
- resolver: bind each type argument in the call's scope
- typecheck: when explicit type args are present, skip argument-driven
  inference and substitute directly; still run constraint checks on each
  type argument; keep argument-driven inference as the default when no
  explicit args are given
- lowering / backend: monomorphize as usual, with bindings taken from the
  explicit args

Editor:

- LSP hover on `pick[int]` shows the monomorphized signature directly
- LSP completion inside `[...]` offers types in scope, not identifiers
- LSP diagnostics cite the explicit type-arg position when a constraint
  check fails
- tree-sitter: add a `generic_call` node in grammar.js; disambiguate from
  existing array/index access; add a highlight rule for the generic
  argument bracket; extend locals/symbols queries if needed
- tree-sitter tests: parse fixtures that mix explicit generic calls with
  ordinary indexing to prove disambiguation holds

Tests:

- positive: call a generic routine with explicit `[int]`
- positive: call a constrained generic routine with explicit `[Rect]`
- positive: disambiguate a call that inference cannot resolve without
  explicit args
- negative: explicit type arg fails the standard constraint
- parser test: `a[0]` still parses as index access, `pick[int](a)` parses
  as a generic call

Docs:

- add an "explicit instantiation" section to
  `book/src/500_items/500_generics.md`
- frame it as an escape hatch, not as the default

Tracked slices:

- [x] I1. Parser and AST for explicit generic-call syntax. Uses
  `name::[TypeArgs](args)` turbofish so the parser never has to guess
  between an index-invoke `xs[i](v)` and a generic call.
- [x] I2. Typecheck substitution and constraint check at explicit args
  (arity mismatch, constraint-failure diagnostics).
- [x] I3. Lowering / backend verification — the turbofish form falls
  through the existing generic-routine monomorphization path; the
  `examples/generic_turbofish_m1` example builds and runs.
- [x] I4. LSP hover / completion / diagnostics. `generic_turbofish_m1`
  is in the LSP open/document-symbols lists (verified by
  `lsp_server_opens_real_model_example_packages_cleanly` and
  `lsp_server_returns_document_symbols_for_real_example_roots`).
- [x] I5. Tree-sitter grammar / queries with turbofish
  disambiguation. Absorbed into P3.
- [x] I6. Positive example (`examples/generic_turbofish_m1`) and
  parser/typecheck negative tests for arity and constraint failures.


# 7. Workstream J: Constrained Generic Types

Goal: `typ Container(T: geo): rec = { var item: T; }` becomes a real
declaration, a real instantiation, and a conformance-checked type.

Why it fits:

- Rust has `struct Container<T: Trait> { ... }`
- monomorphizes identically to constrained generic routines, which FOL
  already supports
- mirroring routines to types is a small, symmetric step

Work:

- parser: already parses `(T: constraint)` on types — verify
- resolver: bind `T` and the constraint inside the type scope
- typecheck: extend `validate_generic_bindings_against_constraints` so it
  runs on generic type instantiation as well as routine calls; emit the
  same diagnostic shape used for constrained generic routines; constraint-
  check at each explicit instantiation, including nested cases; constraint-
  check inside generic routines that instantiate another generic type with
  the receiver's `T`
- lowering / backend: emit Rust `struct Container<T: RustTraitFor_geo>`,
  where the trait for `geo` is whatever protocol standards already lower
  to; verify nested and cross-file forms

Editor:

- LSP hover on `Container[Plain]` at a failure site shows the failing
  constraint
- LSP completion inside `[...]` in instantiation context prefers types
  that satisfy the constraint
- LSP diagnostics cite the explicit type arg, not the receiver
- tree-sitter: verify `generic_param` covers `T: Constraint` inside type
  declarations, not only routines; add highlight for the constraint if not
  already distinct

Tests:

- positive: `typ Container(T: geo): rec` with a conforming type argument
- positive: nested
  `typ Outer(T: geo): rec = { var inner: Container[T]; }`
- positive: cross-file version
- negative: instantiate with a non-conforming type
- negative: use inside a routine argument with a non-conforming type

Docs:

- update `book/src/500_items/500_generics.md` to show constrained generic
  types alongside constrained generic routines

Tracked slices:

- [x] J1. Typecheck constraint validation on generic type instantiation
  (direct, imported constraint, nested instantiation).
- [x] J2. Lowering / backend verified via the
  `examples/generic_type_constrained_m1m2` example which builds and
  runs end to end.
- [x] J3. LSP hover / completion / diagnostics.
  `generic_type_constrained_m1m2` is in the LSP open/document-symbols
  lists.
- [x] J4. Tree-sitter queries audit. Absorbed into P4.
- [x] J5. Positive example plus imported-constraint and nested-
  instantiation negative typecheck tests.


# 8. Workstream K: Default Standard Implementations

Goal: `std geo: pro = { fun area(): int = { return 1; }; }` lowers and
runs, and is inherited by conformers that do not provide their own `area`.

Why it fits:

- Rust has trait default methods
- it is purely static substitution: the default body is inserted at each
  monomorphization site where the conformer has no exact match
- zero runtime dispatch, zero OO

Work:

- parser: already parses a default body inside a standard — verify
- resolver: treat the default body as a lowered routine whose receiver is
  the conforming type at monomorphization time; `self` binds to the
  conformer
- typecheck: delete the rejection at `decls.rs:687`; at conformance check
  time, if the conformer has an exact-match receiver routine, use it,
  otherwise fall back to the default body; keep the exact-match ambiguity
  rule (two matching conformer routines is still an error)
- lowering: emit each default body as a Rust free function parameterized
  over the conforming type, inlined at each monomorphization site
  (preferred over Rust trait default methods, because it guarantees FOL
  never emits a Rust surface that could be dynamically dispatched later)
- backend: verify the inlined form across multi-standard conformance and
  multi-requirement conformance

Editor:

- LSP hover on a default routine shows the default body and marks it as
  "default"
- LSP hover on a conformer that uses the default marks the call site as
  "inherited"
- LSP go-to-definition on an inherited call site leads to the default body
  in the standard
- LSP go-to-definition on an overriding call site leads to the conformer
  routine
- tree-sitter: already parses routine bodies inside standards — verify and
  add a highlight distinction for default bodies if it helps readers

Tests:

- positive: conformer with no `area` inherits the default
- positive: conformer with its own `area` overrides the default
- positive: generic routine constrained by the standard calls `area` and
  compiles against both an overrider and an inheritor
- negative: conformer with two ambiguous matching routines still errors

Docs:

- `book/src/500_items/400_standards.md`: promote default bodies from
  deferred to current
- emphasize that defaults are compile-time substitution, never dispatch

Tracked slices:

- [x] K1. Typecheck tracks `has_default_body` on each required routine
  and lets conformers omit the routine when a default exists. Method
  resolution falls back to the default body when no exact conformer
  routine matches.
- [x] K2. Lowering descends into `std` decl bodies so default bodies
  are emitted as regular routines, and method-call lowering skips
  prepending the receiver arg when the callee has no receiver slot.
- [x] K3. LSP hover / definition / diagnostics for default bodies.
  `standards_default_body_m2` is in the LSP open/document-symbols
  lists.
- [x] K4. `examples/standards_default_body_m2` builds and runs and
  typecheck tests pin inheritance, override, signature mismatch, and
  conformer-less inheritance.


# 9. Workstream L: Generic Error Types

Goal: `fun parse(T)(input: str): T / ParseError[T]` compiles and runs.

Why it fits:

- Rust has `Result<T, E>` with generic `T` and `E`
- monomorphization handles the substitution completely
- no new runtime behavior

Work:

- parser: already accepts the form — verify
- resolver: verify `err[T]` uses the same `T` binding as the routine's
  generic parameter scope
- typecheck: delete the rejection at `decls.rs:593`; verify interaction
  with `check(...)`, `report`, `||` fallback, and the `value!` postfix
  unwrap — each must pass the generic parameter through cleanly
- lowering: emit `Result<T, E>` with monomorphized `T` and `E`
- backend: verify the error-shell container path for `err[T]`

Editor:

- LSP hover on a generic recoverable routine shows the generic error type
- LSP diagnostics on error paths cite the generic error parameter
- tree-sitter: verify `fun ...(): T / E` parses with both `T` and `E` as
  generic references; adjust queries if needed

Tests:

- positive: generic recoverable routine with `check(...)`
- positive: generic recoverable routine with `||` fallback
- positive: generic recoverable routine with `!` postfix unwrap
- negative: inference failure when the error generic is independent of
  the argument types

Docs:

- update `book/src/650_errors/` to show generic error types
- update `book/src/500_items/500_generics.md` to drop the deferred note

Tracked slices:

- [x] L1. Typecheck lift and generic substitution through error paths.
  The routine signature lowers cleanly and call sites flow monomorphized
  bindings through return and error types.
- [x] L2. Lowering now pulls the call-site's recorded recoverable effect
  for the error type on free function calls, so the emitted Rust
  `FolRecover<T, E>` sees the substituted `T` and `E` instead of the
  unsubstituted generic parameter.
- [x] L3. LSP hover / diagnostics audit. `generic_error_m1m2` is in
  the LSP open/document-symbols lists.
- [x] L4. Tree-sitter queries audit. Absorbed into P6.
- [x] L5. `examples/generic_error_m1m2` builds and runs the full
  generic recoverable routine path through `check(...)`.


# 10. Workstream M: Blueprint Standards As Static Field Contracts

Goal: `std sized: blu = { var size: int; }` becomes a real static contract
meaning "a conformer must declare a field `size: int`", usable as a
generic constraint: `fun measure(T: sized)(value: T): int = { return value.size; }`.

Why it fits:

- structurally identical to protocol standards, just over fields instead
  of routines
- conformance is compile-time only: checked at the type declaration
- the conformer declares the field itself, so nothing is ever inherited
- no runtime, no dispatch, no boxing, no inheritance
- Rust does not have a direct equivalent as a single feature, but the
  lowering strategy is trivial: inline field access at each monomorphized
  use site
- this gives FOL compile-time leverage that feels native to the
  "lean on compile time" direction

Work:

- parser: already parses `std X: blu = { var name: type; }` — verify
- resolver: bind blueprint standards as first-class standard symbols in
  the same scope as protocol standards
- typecheck: delete the rejection at `decls.rs:240`; for each type
  conforming to a blueprint, walk its declared fields and require that
  each blueprint requirement is matched by a same-name field with a
  compatible type; emit precise diagnostics for missing or mismatched
  fields, mirroring the protocol routine diagnostics
- typecheck: allow blueprint standards as generic constraints; when a
  constrained generic routine accesses a field named by a blueprint
  requirement, allow it (static field access is already monomorphized)
- typecheck: reject blueprint standards used as value or parameter types
  with the same permanent rejection shape introduced in G2
- lowering: blueprint conformance has no runtime effect on the conformer;
  generic routines constrained by a blueprint lower as ordinary generic
  routines whose monomorphized body references the field directly
- backend: prefer inlined field access at each monomorphized use site
  over an emitted Rust trait — this keeps the guarantee of zero dispatch

Editor:

- LSP hover on a blueprint conformer lists the required fields with their
  types
- LSP diagnostics on a missing field cite the conforming type, not the
  standard
- LSP completion on `value.` inside a routine constrained by a blueprint
  offers the fields required by the blueprint
- LSP go-to-definition on a blueprint field inside a generic body leads to
  the blueprint requirement declaration
- tree-sitter: verify `std_decl` with kind `blu` is parsed; verify
  `standard_requirement` allows field declarations; add a highlight rule
  for the `blu` keyword; extend locals / symbols to cover blueprint fields

Tests:

- positive: single-field blueprint, conformer has the field
- positive: multi-field blueprint
- positive: blueprint as constraint on a generic routine that accesses
  the field
- positive: blueprint and protocol together on one type
- negative: conformer missing a required field
- negative: conformer has the field with the wrong type
- negative: blueprint used as a value type (same rejection shape as G2)

Docs:

- update `book/src/500_items/400_standards.md` to promote blueprints from
  "future" to current contract
- make it explicit that blueprint conformance is purely compile-time
  field shape

Tracked slices:

- [x] M1. Typecheck: blueprint conformance at type declaration time.
  Blueprint standards ship a new `required_fields` list on `TypedStandard`;
  conformance checks walk the conformer's record fields and emit precise
  missing-field / wrong-type / not-a-record diagnostics.
- [x] M2. Blueprints work as generic constraints — a routine with
  `fun measure(T: sized)(value: T)` accepts a conformer whose declared
  record satisfies the field requirements. End-to-end build runs.
- [x] M3. Lowering / backend: blueprints do not synthesize any runtime
  surface, so conforming types pass through the existing record + generic
  routine lowering path unchanged. The `standards_blueprint_m2` example
  builds and runs end to end.
- [x] M4. LSP hover / completion / diagnostics for blueprints.
  `standards_blueprint_m2` is in the LSP open/document-symbols lists and
  opens without editor diagnostics.
- [x] M5. Tree-sitter grammar / queries audit. `standard_field_requirement`
  covers blueprint `var` members; the blueprint example parses with zero
  ERROR nodes under the CLI-validated grammar, and the highlight queries
  compile against the generated parser.
- [x] M6. Positive example (`examples/standards_blueprint_m2`),
  negative example (`fail_standard_blueprint_m2` refitted to a
  missing-field failure), and typecheck tests for matching/missing/
  wrong-type conformer fields.


# 11. Workstream N: Extended Standards

Goal: `std drawable: ext = { fun draw(): int; var color: int; }` becomes a
real contract that requires both a routine and a field on the conformer.

Why it fits:

- once blueprints land, extended standards are just "protocol + blueprint
  stacked on one symbol"
- no new semantics beyond the union of the two
- no new lowering strategy

Depends on: Workstream M.

Work:

- parser: already parses `std X: ext` — verify
- resolver: treat `ext` as a standard kind that can hold both routine and
  field requirements
- typecheck: delete the rejection at `decls.rs:243`; route each requirement
  to the protocol rules or the blueprint rules depending on its shape;
  keep diagnostics cited to the individual failing requirement, not a
  vague "does not satisfy"
- lowering: each routine requirement lowers as in protocol standards; each
  field requirement lowers as in blueprint standards
- backend: verify composition across mixed requirements

Editor:

- LSP hover on an `ext` standard shows both sides
- LSP diagnostics cite each side independently
- tree-sitter: verify `standard_requirement` supports routine and field
  members under `ext`; update queries if highlight/locals need new cases

Tests:

- positive: one routine plus one field, conformer satisfies both
- positive: `ext` as generic constraint
- negative: conformer missing the routine side
- negative: conformer missing the field side

Docs:

- update `book/src/500_items/400_standards.md` to document extended
  standards as the union of protocol + blueprint

Tracked slices:

- [x] N1. Typecheck extended conformance dispatches each member to the
  protocol or blueprint path based on its AST shape; the existing
  `required_routines` and `required_fields` lists cover both sides.
- [x] N2. Lowering / backend: extended standards lower as the union of
  their routine and field requirements — no new backend surface.
  `examples/standards_extended_m2` builds and runs.
- [x] N3. LSP hover / diagnostics for extended standards.
  `standards_extended_m2` is in the LSP open/document-symbols lists and
  opens without editor diagnostics.
- [x] N4. Tree-sitter queries audit. Extended standards reuse
  `standard_requirement` plus `standard_field_requirement`; the extended
  example parses with zero ERROR nodes under the CLI-validated grammar.
- [x] N5. Typecheck tests pin accept/missing-routine/missing-field
  cases. Example `standards_extended_m2` demonstrates positive
  end-to-end flow through procedural method dispatch.


# 12. Workstream O: Generic Standards

Goal: `std Iterator[T]: pro = { fun next(): T; }` becomes a parameterizable
standard. Conformers parameterize as
`typ IntIter()(Iterator[int]): rec = { ... }`.

Why it fits:

- Rust has generic traits (`trait Iterator<Item>` or
  `trait Iterator { type Item; }`)
- monomorphization still handles everything
- gives FOL real expressiveness: "this type is an iterator of X" can be
  stated statically without dispatch

Important distinction:

- this workstream parameterizes the **standard itself**
- it does **not** reintroduce per-routine generics inside a standard body
  (those are removed in G5)
- individual required routines stay concrete, but they may reference the
  standard's own generic parameters

Work:

- parser: add a generic parameter slot on `std` declarations; extend
  conformance headers to accept generic arguments on standard names;
  extend constraint syntax to accept generic arguments on standard names
  (`fun f(T: Iterator[int])(v: T)`)
- AST: new generic parameter list on standard declarations; generic
  argument list on standard references in conformance and constraints
- resolver: bind the standard's generic parameters in the standard scope;
  substitute at conformance-header time and at constraint-check time
  with the supplied arguments
- typecheck: conformance check substitutes the generic arguments into each
  required routine signature, then runs the existing exact-match rule;
  generic routine constraints accept `Iterator[int]` and check with the
  substituted signatures
- lowering: prefer inlined substitution over emitting a Rust generic
  trait, again to keep zero-dispatch guarantees
- backend: verify that each conformer declares exactly one concrete
  instantiation, so monomorphization stays bounded

Editor:

- LSP hover on `Iterator[int]` shows the substituted routine signatures
- LSP diagnostics on conformance failures cite substituted signatures,
  not unsubstituted ones
- tree-sitter: extend `std_decl` to accept a generic parameter slot;
  extend the type-contract claim rule to accept generic arguments on the
  standard name; update highlights / locals / symbols
- tree-sitter tests: parse fixtures for generic standards in declaration,
  conformance header, and constraint positions

Tests:

- positive: `Iterator[int]` with an int-producing conformer
- positive: `Iterator[str]` with a str-producing conformer
- positive: generic routine constrained by `Iterator[T]` where `T` is the
  outer routine's own generic parameter
- positive: explicit generic call `iterate[int](source)` (depends on I)
- negative: conformer claims `Iterator[int]` but the routine returns `str`
- negative: arity mismatch on the standard's generic parameters

Docs:

- `book/src/500_items/400_standards.md`: add generic standards
- cross-reference `book/src/500_items/500_generics.md`

Tracked slices:

- [x] O1. Parser / AST for generic parameters on `std` declarations:
  `std Iterator(T): pro = { fun next(): T; }`. Uses the existing
  type-generic-header parser; `lookahead_is_std_decl` skips the
  generic header before the colon so the parser dispatches cleanly.
- [x] O2. Resolver binding: the standard scope now inserts generic
  parameter symbols so routine signatures inside the body can reference
  them. Contract references like `Iterator[int]` split off the base
  name before the standard-symbol lookup.
- [x] O3. Typecheck substitutes each claim's type arguments into the
  standard's routine and field requirements before running the
  existing exact-match / blueprint-field rules. Arity-mismatch at the
  conformance header produces a clean diagnostic.
- [x] O4. Lowering / backend: generic standards use the existing
  record + receiver-routine lowering path. The
  `examples/standards_generic_m2` example builds and runs end to end.
- [x] O5. LSP hover / diagnostics for generic standards.
  `standards_generic_m2` is in the LSP open/document-symbols lists and
  opens without editor diagnostics.
- [x] O6. Tree-sitter grammar / queries updates. `std_decl` carries the
  `generics` field; the generic-standard example parses with zero ERROR
  nodes under the CLI-validated grammar.
- [x] O7. Typecheck tests pin accept / wrong-return / arity-mismatch
  cases. `examples/standards_generic_m2` demonstrates positive
  end-to-end execution with a substituted concrete routine.


# 13. Workstream P: Editor And Tree-Sitter Hardening (Cross-Cutting)

This is not a phase after G through O. It runs **in the same change set**
as each workstream above.

Per-feature editor requirements:

- positive LSP test: open each new positive example, run hover and
  go-to-definition over the new surface, assert sensible results
- negative LSP test: open each new negative example, assert the diagnostic
  text matches the new message wording
- tree-sitter parse test: each new positive example parses to the expected
  node shape
- tree-sitter parse test: each removed surface **fails to parse** — not
  "parses and is rejected later", but fails at the grammar level

Primary files:

- `lang/tooling/fol-editor/tree-sitter/grammar.js`
- `lang/tooling/fol-editor/queries/fol/highlights.scm`
- `lang/tooling/fol-editor/queries/fol/locals.scm`
- `lang/tooling/fol-editor/queries/fol/symbols.scm`
- `lang/tooling/fol-editor/src/tree_sitter.rs`
- `lang/tooling/fol-editor/src/lsp/tests/example_models.rs`
- `lang/tooling/fol-editor/src/lsp/tests/navigation.rs`
- any fixtures under `lang/tooling/fol-editor/tests/`

Tracked slices:

- [x] P1. Editor / tree-sitter updates shipped alongside G. Removed
  `imp_decl` rule, receiver/capture slots inside `standard_requirement`.
- [x] P2. Editor / tree-sitter updates shipped alongside H. Generic
  receiver routines parse through the existing receiver rule.
- [x] P3. Editor / tree-sitter updates shipped alongside I. Turbofish
  `name::[T](args)` added as `turbofish_type_args` under `call_expr`,
  with `qualified_path` marked `prec.left` to avoid `::`/turbofish
  ambiguity.
- [x] P4. Editor / tree-sitter updates shipped alongside J.
  Constrained generic types parse through the existing `generic_param`
  with `: constraint` shape.
- [x] P5. Editor / tree-sitter updates shipped alongside K. Default
  bodies on required routines parse through an optional `= block`
  trailer on `standard_requirement`.
- [x] P6. Editor / tree-sitter updates shipped alongside L. Generic
  error types use the existing error-type rule with generic parameter
  references.
- [x] P7. Editor / tree-sitter updates shipped alongside M. Added
  `standard_field_requirement` for blueprint `var` members inside
  `standard_block`.
- [x] P8. Editor / tree-sitter updates shipped alongside N. Extended
  standards reuse `standard_requirement` and `standard_field_requirement`
  inside the same block.
- [x] P9. Editor / tree-sitter updates shipped alongside O. Added
  `generics` field on `std_decl` plus tree-sitter query coverage for
  all new V2 example roots. Refreshed pre-existing LSP example
  fixtures to match the current V2 contract.

Rule:

- P is listed separately only for plan visibility
- do not use it as an excuse to ship compiler changes first and editor
  changes later


# 14. Recommended Order

1. Workstream G (remove surfaces we will not build).
2. Workstream H (generic receiver types). Completes generic types.
3. Workstream I (explicit generic-call syntax). Escape hatch used by J
   and O.
4. Workstream J (constrained generic types). Mirrors constrained generic
   routines, small symmetric step.
5. Workstream K (default standard implementations). High ergonomics,
   strictly static substitution.
6. Workstream L (generic error types). Natural after H.
7. Workstream M (blueprint standards). New contract shape.
8. Workstream N (extended standards). Depends on M.
9. Workstream O (generic standards). Most invasive, last.

Workstream P runs alongside each of the above in the same change set.


# 15. Non-Goals Restated

The following are permanently out of scope for this plan and for FOL:

- object-model semantics of any kind
- classes, inheritance, base types
- `dyn` / trait objects / runtime witness tables
- standards used as values or parameter types
- global solver-style inference
- implicit runtime dispatch through any surface
- first-class generic routine values via type erasure
- boxing or heap indirection used to fake polymorphism

If any workstream above starts to require one of these, it stops.


# 16. Completion Rule

This plan is complete only when:

- every G removal is gone from parser, AST, resolver, typecheck rejections,
  tree-sitter grammar, LSP queries, docs, and examples
- every H through O feature ships end to end in compiler, lowering,
  backend, LSP, tree-sitter, docs, and examples, or is explicitly and
  narrowly re-scoped in this plan with a reason
- no "planned for a future release" message in the compiler refers to a
  feature the project has already chosen not to build
- the V2 chapters in the book describe the resulting language exactly,
  with no leftover pre-removal wording
- the project can honestly say: "this is the V2 we want — compile-time
  heavy, Rust-native in the backend, and with no OO anywhere"
