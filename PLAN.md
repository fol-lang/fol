# V2 Continuation Plan

This plan tracks the next `V2` work that still makes sense for FOL after the
frozen full-`V2` contract landed.

The goal here is not to reopen the broad historical `V2` idea.
The goal is to deepen the current language direction:

- static polymorphism
- static contracts
- procedural call binding
- monomorphized code generation
- no object model
- no runtime witness/dictionary dispatch layer

This plan is intentionally aligned with a Rust/Zig-like direction rather than
an OO/trait-object direction.


# 1. Guardrails

These are non-negotiable unless the project explicitly chooses a different
language direction later.

Keep:

- generic routines
- generic types
- protocol standards
- standards-as-constraints
- static conformance checking
- monomorphized lowering/backend emission
- procedural call binding

Do not add as part of this plan:

- blueprint standards
- extended standards
- standards as ordinary value/object types
- object-style dispatch semantics
- runtime witness/dictionary/vtable systems
- broad global solver-style inference
- hidden dynamic dispatch behind standards

If future work wants any of that, it should be a separate explicit roadmap, not
smuggled into `V2`.


# 2. Current Truth

The currently shipped full `V2` contract is:

- executable generic routines
- generic types
- executable protocol standards
- standards-as-constraints

The remaining good `V2` work is therefore not about widening into object-model
features. It is about strengthening the current static model.


# 3. Exit Criteria

This plan is complete when all of the following are true:

1. Tree-sitter and editor support honestly match the shipped `V2` syntax.
2. Generic-type syntax is mirrored in tooling, not only in compiler truth.
3. Positive executable `V2` examples are covered by editor/tree-sitter tests,
   not only by compiler/runtime integration tests.
4. The remaining narrow generic and standards limits are either:
   - implemented end to end, or
   - explicitly documented and tested as intentional current limits.
5. Book/docs/AGENTS/editor docs all describe the same current `V2` reality.


# 4. Workstream A: Tooling Parity For Real V2 Syntax

Goal:

- make editor and tree-sitter support match the real shipped `V2` syntax,
  especially generic-type instantiation

Why this matters:

- today the compiler accepts more real `V2` generic-type surface than
  tree-sitter mirrors
- that weakens the “feature complete” claim even when compiler/runtime are
  correct

Required work:

- add tree-sitter support for user generic-type instantiation syntax such as
  `Box[int]`
- verify nested generic-type forms parse under tree-sitter
- update highlight/locals/symbol queries if the syntax shape changes
- add real tests against shipped positive generic-type examples

Primary files:

- `lang/tooling/fol-editor/tree-sitter/grammar.js`
- `lang/tooling/fol-editor/queries/fol/highlights.scm`
- `lang/tooling/fol-editor/queries/fol/locals.scm`
- `lang/tooling/fol-editor/queries/fol/symbols.scm`
- `lang/tooling/fol-editor/src/tree_sitter.rs`

Acceptance:

- shipped positive generic-type examples parse under tree-sitter
- queries still produce honest symbols/highlights/locals
- no shipped generic-type syntax exists only in compiler truth

Tracked slices:

- [x] A1. Add tree-sitter support for generic-type instantiation syntax.
- [x] A2. Audit query files against the new generic-type tree shape.
- [x] A3. Add tree-sitter coverage for shipped positive executable generic-type
  examples.


# 5. Workstream B: Editor Coverage For Real Positive V2 Examples

Goal:

- make the editor mirror exercise the real shipped positive `V2` examples, not
  only semantic or negative fixtures

Why this matters:

- the docs currently imply broader editor awareness than the tests prove
- positive executable examples should flow through hover/definition/diagnostics
  too

Required work:

- add editor-open/hover/definition coverage for:
  - `examples/generic_type_exec_m1m2`
  - `examples/generic_standard_constraint_m1m2`
  - `examples/standards_protocol_m2`
- verify no editor-only stale boundaries survive on those example roots
- narrow docs if any support intentionally remains absent

Primary files:

- `lang/tooling/fol-editor/src/lsp/tests/example_models.rs`
- `lang/tooling/fol-editor/src/lsp/tests/navigation.rs`
- `docs/editor-sync.md`
- `book/src/050_tooling/500_lsp.md`

Acceptance:

- positive executable `V2` examples are covered by editor tests
- editor docs describe the exact real supported example set

Tracked slices:

- [x] B1. Add editor coverage for the positive executable generic-type example.
- [x] B2. Add editor coverage for the positive executable constrained-generic example.
- [x] B3. Re-audit editor docs so they match the real tested example matrix.


# 6. Workstream C: Narrow Generic Improvements

Goal:

- improve the current static generics model without widening into broad solver
  or runtime-dispatch semantics

Why this matters:

- the current generic core is real, but several narrow limitations remain
- some of those limits are reasonable; some should be lifted if they can stay
  explicit and monomorphized

Candidate work:

- improve diagnostics for underconstrained generics so the failure mode is more
  actionable without changing the inference model
- evaluate whether generic receiver types can be supported cleanly under the
  current lowering/backend strategy
- evaluate whether generic recoverable error types can be supported cleanly
  without introducing hidden complexity
- improve nested generic type composition and error reporting

Non-goals:

- broad contextual inference
- implicit global constraint solving
- first-class generic routine values unless they can stay explicit and static

Primary files:

- `lang/compiler/fol-typecheck/src/decls.rs`
- `lang/compiler/fol-typecheck/src/exprs/calls.rs`
- `lang/compiler/fol-lower/src/decls/routine_decls.rs`
- `lang/compiler/fol-lower/src/decls/type_decls.rs`
- `test/typecheck/test_typecheck_generics_m1.rs`
- `test/typecheck/test_typecheck_generic_types_v2.rs`

Acceptance:

- every lifted generic limit is covered in parser/resolver/typecheck/lowering
  and editor/docs where relevant
- every retained generic limit remains explicitly documented and tested

Tracked slices:

- [x] C1. Improve underconstrained-generic diagnostics without widening inference.
- [x] C2. Decide whether generic receiver types stay a limit or become supported.
- [x] C3. Decide whether generic recoverable error types stay a limit or become supported.
- [ ] C4. Audit nested generic-type composition and improve diagnostics or implementation.


# 7. Workstream D: Static Standards Improvements

Goal:

- make protocol standards better static contracts without turning them into
  runtime objects

Why this matters:

- standards are good for static conformance and constrained generics
- they should become more useful in that role without drifting into OO design

Candidate work:

- improve conformance diagnostics, especially across packages
- improve diagnostics for ambiguous/missing required routines
- evaluate carefully chosen richer required routine signature forms if they
  remain fully static and procedural
- keep standards-as-constraints ergonomics strong and explicit

Non-goals:

- standard values
- dynamic dispatch through standards
- runtime method tables

Primary files:

- `lang/compiler/fol-typecheck/src/decls.rs`
- `lang/compiler/fol-typecheck/src/exprs/calls.rs`
- `lang/compiler/fol-lower/src/decls/standards.rs`
- `test/typecheck/test_typecheck_standards_m2.rs`
- `docs/v2-standards-m2.md`
- `book/src/500_items/400_standards.md`

Acceptance:

- standards become easier to use as static contracts
- no new work implies runtime object semantics
- docs stay explicit that standards remain procedural contracts

Tracked slices:

- [ ] D1. Improve conformance diagnostics for missing and ambiguous required routines.
- [ ] D2. Audit cross-package standards diagnostics and tighten them where weak.
- [ ] D3. Decide whether any richer required routine signature forms should be added.


# 8. Workstream E: Constraint Ergonomics Without Solver Creep

Goal:

- improve standards-as-constraints ergonomics while keeping inference local and
  explicit

Why this matters:

- constrained generics are useful already
- the next improvements should reduce friction, not introduce hidden magic

Candidate work:

- improve diagnostic wording when a type fails a constraint
- improve reporting when multiple candidate standards or conformances are in
  scope
- improve generic-constraint error messages on generic types as well as generic
  routines

Non-goals:

- global typeclass/trait-style search
- inference from unrelated contextual use
- implicit runtime dispatch selection

Primary files:

- `lang/compiler/fol-typecheck/src/exprs/calls.rs`
- `lang/compiler/fol-typecheck/src/decls.rs`
- `test/typecheck/test_typecheck_generics_m1.rs`
- `test/typecheck/test_typecheck_generic_types_v2.rs`
- `test/typecheck/test_typecheck_standards_m2.rs`

Acceptance:

- constrained-generic failures are precise and easy to act on
- diagnostics stay aligned across routines, generic types, and examples

Tracked slices:

- [x] E1. Improve constrained-generic routine diagnostics.
- [x] E2. Improve constrained generic-type instantiation diagnostics.
- [ ] E3. Audit ambiguous-constraint and imported-constraint diagnostics.


# 9. Workstream F: Repo-Wide Contract Honesty

Goal:

- remove remaining stale prose that still describes old pre-landing `V2`
  boundaries

Why this matters:

- compiler/runtime truth is stronger than some docs still claim
- stale prose causes the wrong architectural decisions later

Required work:

- update the standards chapter where it still treats standards-as-constraints as
  future work
- update `AGENTS.md` where it still describes old M1/M2 pre-lowering boundaries
- audit tooling docs that still describe old workspace-symbol limitations if the
  implementation moved beyond them

Primary files:

- `AGENTS.md`
- `book/src/500_items/400_standards.md`
- `book/src/050_tooling/300_editor.md`
- `book/src/050_tooling/500_lsp.md`
- `book/src/050_tooling/600_neovim.md`

Acceptance:

- no top-level guidance file tells contributors that current shipped `V2`
  features are still unimplemented
- tooling docs match actual implementation boundaries

Tracked slices:

- [x] F1. Remove stale pre-landing V2 milestone wording from `AGENTS.md`.
- [x] F2. Fix stale standards-book wording about standards-as-constraints being future work.
- [x] F3. Re-audit tooling docs for stale workspace-symbol/editor boundary claims.


# 10. Recommended Order

Work should proceed in this order:

1. tooling parity for real generic-type syntax
2. editor coverage for positive executable `V2` examples
3. repo-wide contract honesty cleanup
4. narrow generic improvements
5. static standards improvements
6. constraint ergonomics polishing

Reason:

- steps 1 through 3 close the remaining “feature is real in compiler but not
  mirrored honestly” gap
- steps 4 through 6 deepen `V2` while keeping the language on the intended
  static, procedural path


# 11. Completion Rule

This plan is complete only when:

- the shipped `V2` surface is mirrored honestly across compiler, backend,
  runtime, editor, tree-sitter, docs, and examples
- the remaining improvements strengthen static polymorphism and static
  contracts without introducing object-model semantics
- any still-retained limits are explicit and tested rather than accidental
- the project can say “this is the V2 we want” without drifting toward an OO
  language
