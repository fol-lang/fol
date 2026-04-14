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
- generic receiver types on routines
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

- [ ] G1. Remove `imp Self: std = { ... }` implementation blocks.
- [ ] G2. Turn standards-in-type-position into a permanent error.
- [ ] G3. Remove receiver-qualified required routines from `std` bodies.
- [ ] G4. Remove capturing required routines from `std` bodies.
- [ ] G5. Remove per-routine generic required routines from `std` bodies.


# 5. Workstream H: Generic Receiver Types

Goal: `fun (Box[T])get(): T = { return self.value; }` becomes a real,
executable, typed routine.

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

- [ ] H1. Parser/AST/resolver verification and typecheck lift.
- [ ] H2. Lowering / backend emission for generic receiver routines.
- [ ] H3. LSP hover / definition / diagnostics.
- [ ] H4. Tree-sitter grammar / queries audit and coverage.
- [ ] H5. Positive and negative examples plus docs.


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

- [ ] I1. Parser and AST for explicit generic-call syntax.
- [ ] I2. Typecheck substitution and constraint check at explicit args.
- [ ] I3. Lowering / backend verification.
- [ ] I4. LSP hover / completion / diagnostics.
- [ ] I5. Tree-sitter grammar / queries with index disambiguation.
- [ ] I6. Positive and negative examples plus docs.


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

- [ ] J1. Typecheck constraint validation on generic type instantiation.
- [ ] J2. Lowering / backend with trait bounds on generic structs.
- [ ] J3. LSP hover / completion / diagnostics.
- [ ] J4. Tree-sitter queries audit.
- [ ] J5. Positive and negative examples plus docs.


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

- [ ] K1. Typecheck resolves default body at conformance time.
- [ ] K2. Lowering / backend emission for default bodies as inlined
  monomorphization.
- [ ] K3. LSP hover / definition / diagnostics for default bodies.
- [ ] K4. Positive and negative examples plus docs.


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

- [ ] L1. Typecheck lift and generic substitution through error paths.
- [ ] L2. Lowering / backend emission with generic error types.
- [ ] L3. LSP hover / diagnostics audit.
- [ ] L4. Tree-sitter queries audit.
- [ ] L5. Positive and negative examples plus docs.


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

- [ ] M1. Typecheck: blueprint conformance at type declaration time.
- [ ] M2. Typecheck: blueprints as generic constraints with static field
  access.
- [ ] M3. Lowering / backend emission with inlined field access.
- [ ] M4. LSP hover / completion / diagnostics for blueprints.
- [ ] M5. Tree-sitter grammar / queries audit.
- [ ] M6. Positive and negative examples plus docs.


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

- [ ] N1. Typecheck extended conformance dispatches to protocol and
  blueprint rules.
- [ ] N2. Lowering / backend verification for extended standards.
- [ ] N3. LSP hover / diagnostics for extended standards.
- [ ] N4. Tree-sitter queries audit.
- [ ] N5. Positive and negative examples plus docs.


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

- [ ] O1. Parser / AST for generic parameters on `std` declarations.
- [ ] O2. Resolver binding and substitution at conformance-header and
  constraint sites.
- [ ] O3. Typecheck with substituted signatures and constrained generics.
- [ ] O4. Lowering / backend emission with inlined substitution.
- [ ] O5. LSP hover / diagnostics for generic standards.
- [ ] O6. Tree-sitter grammar / queries updates.
- [ ] O7. Positive and negative examples plus docs.


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

- [ ] P1. Editor / tree-sitter updates shipped alongside G.
- [ ] P2. Editor / tree-sitter updates shipped alongside H.
- [ ] P3. Editor / tree-sitter updates shipped alongside I.
- [ ] P4. Editor / tree-sitter updates shipped alongside J.
- [ ] P5. Editor / tree-sitter updates shipped alongside K.
- [ ] P6. Editor / tree-sitter updates shipped alongside L.
- [ ] P7. Editor / tree-sitter updates shipped alongside M.
- [ ] P8. Editor / tree-sitter updates shipped alongside N.
- [ ] P9. Editor / tree-sitter updates shipped alongside O.

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
