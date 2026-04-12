# V2 Audit Follow-Up Plan

This plan replaces the earlier "full V2 is complete" checklist.

That earlier plan was good enough to drive the narrow compiler-side milestone
work, but it is no longer an honest tracker for the project's actual state
after the repo-wide audit.

The current audit result is:

- the frozen V2 compiler contract is mostly implemented
- the repo is not yet fully aligned end to end
- the remaining gaps are mostly:
  - tooling and tree-sitter parity
  - stale tests and stale example labeling
  - ambiguous backend/runtime meaning for lowered standards metadata
  - portability gaps between documented examples and harness-only setup

This file tracks the remaining work required before V2 can be described as
fully implemented across the project, not just in the compiler core.


# 1. Current Truth

The currently frozen shipped V2 contract is still:

- executable generic routines
- generic types
- protocol standards
- standards-as-constraints

That narrow contract is defined in:

- `docs/v2-full-contract.md`
- `docs/v2-runtime-strategy.md`

The audit also confirmed that broader historical V2 ideas are still deferred
and are not part of the current target:

- blueprint standards
- extended standards
- standards as ordinary concrete value types
- standards-driven dispatch
- broader inference / solver-style behavior
- object-style dispatch semantics

Those deferred items should stay out of current claims and out of completion
criteria for this plan.


# 2. Exit Criteria

V2 is only "fully implemented" when all of the following are true together:

1. Compiler truth is implemented and tested.
2. Backend/runtime meaning is honest and non-ambiguous.
3. Tree-sitter and editor behavior match the shipped V2 syntax and semantics.
4. Positive and negative examples are labeled correctly and run the documented
   way.
5. Book/docs/PLAN/example inventory all describe the same product reality.

If one of those fails, V2 is still only partially complete at the repo level.


# 3. Workstream A: Contract Honesty And Plan Cleanup

Goal:

- remove stale statements that still describe old V2 boundaries
- make docs, plan text, and milestone notes agree on what is real today

Required work:

- remove stale wording that still says standards stop before lowering/backend
  if the current code/tests claim executable protocol examples
- rewrite milestone docs that still describe old boundaries after the code
  moved past them
- ensure `PLAN.md`, milestone notes, and book chapters no longer disagree

Primary files:

- `PLAN.md`
- `docs/v2-full-contract.md`
- `docs/v2-generics-m1.md`
- `docs/v2-standards-m2.md`
- `book/src/500_items/500_generics.md`
- `book/src/500_items/400_standards.md`

Acceptance:

- no current doc says "not lowered/executable" for a surface that current
  tests already exercise positively
- no current doc implies broader V2 features that were explicitly cut

Tracked slices:

- [x] A1. Replace stale completed-plan assertions with follow-up-plan checks.
- [x] A2. Retag generic milestone docs and book entries that still describe
  `examples/fail_generic_type_m1` as a negative boundary.
- [ ] A3. Reconcile standards/runtime docs so they do not imply richer runtime
  semantics than the backend currently implements.


# 4. Workstream B: Tree-Sitter Parity For Shipped V2

Goal:

- make tree-sitter honestly support the shipped V2 syntax that the compiler
  accepts, or narrow the shipped claim if that support is intentionally absent

Known audit findings:

- `lang/tooling/fol-editor/tree-sitter/grammar.js` is not yet fully shaped for
  the shipped V2 generic/contract forms
- a V2 standards tree-sitter test currently fails
- one supposed V2 audit function is present without `#[test]`, so it does not
  actually gate anything

Required work:

- audit and fix grammar support for shipped generic type forms
- audit and fix grammar support for shipped protocol standard/conformance forms
- update highlight/locals/symbol queries where syntax shape changes
- convert the non-running audit helper into a real test or delete it and add a
  real replacement
- ensure V2 example inventory checks are testing what the docs claim

Primary files:

- `lang/tooling/fol-editor/tree-sitter/grammar.js`
- `lang/tooling/fol-editor/queries/fol/highlights.scm`
- `lang/tooling/fol-editor/queries/fol/locals.scm`
- `lang/tooling/fol-editor/queries/fol/symbols.scm`
- `lang/tooling/fol-editor/src/tree_sitter.rs`

Acceptance:

- the shipped V2 example set parses under tree-sitter
- the failing V2 standards tree-sitter test passes
- there is no fake audit coverage hidden behind a missing `#[test]`

Tracked slices:

- [x] B1. Turn the non-running V2 tree-sitter audit helper into a real test.
- [ ] B2. Fix shipped V2 standards tree-sitter query coverage so the failing
  standards example test passes honestly.


# 5. Workstream C: Editor Regression And Mirror Cleanup

Goal:

- remove stale editor expectations that still reflect old V2 boundaries

Known audit findings:

- at least one editor navigation regression still expects generic types to be
  unsupported
- that regression is malformed enough to fail earlier than the intended
  semantic boundary
- the editor docs are narrower and mostly honest, but the test matrix is not
  fully synchronized with current compiler truth

Required work:

- repair malformed V2 editor regression fixtures
- update generic-type hover/definition/typecheck expectations to current truth
- verify standards example hover/definition behavior against the now-shipped
  surface
- keep the editor mirror honest where support is intentionally narrow

Primary files:

- `lang/tooling/fol-editor/src/lsp/tests/navigation.rs`
- `lang/tooling/fol-editor/src/lsp/tests/example_models.rs`
- `test/integration_tests/integration_cli_typecheck.rs`
- `test/integration_tests/integration_editor_and_build.rs`
- `docs/editor-sync.md`
- `book/src/050_tooling/500_lsp.md`

Acceptance:

- there are no editor regressions asserting old V2 boundaries that are no
  longer true
- editor tests fail only for real unsupported behavior, not malformed fixtures
- editor docs match current shipped support

Tracked slices:

- [x] C1. Repair the malformed editor regression that still expects generic
  types to be unsupported.
- [ ] C2. Re-audit editor docs/tests against the currently shipped V2 subset
  and remove stale narrow-boundary wording.


# 6. Workstream D: Backend/Runtime Meaning Of Standards

Goal:

- remove ambiguity around what lowered standards/conformance data actually
  means at execution time

Known audit findings:

- typecheck enforces standards/conformance legality
- lowering carries protocol/conformance metadata
- backend/runtime do not currently appear to consume that metadata as a richer
  execution model
- runtime docs still describe standards/generics as out of scope in places

This workstream needs a hard project decision first:

- either standards remain a compile-time-only legality/selection mechanism
- or backend/runtime must gain explicit support that consumes the lowered
  metadata

The repo should not continue in an ambiguous middle state.

Required work:

- document the chosen semantics explicitly
- if compile-time-only:
  - remove broader runtime wording
  - add tests proving the intended compile-time-only boundary
- if runtime-significant:
  - implement backend/runtime consumption of lowered standards metadata
  - add execution tests that prove the runtime effect is real
- align runtime crate docs with the chosen truth

Primary files:

- `lang/compiler/fol-typecheck/src/decls.rs`
- `lang/compiler/fol-typecheck/src/exprs/calls.rs`
- `lang/compiler/fol-lower/src/decls/standards.rs`
- `lang/compiler/fol-lower/src/session.rs`
- `lang/execution/fol-backend/src/...`
- `lang/execution/fol-runtime/src/lib.rs`
- `docs/v2-runtime-strategy.md`
- `docs/v2-standards-m2.md`

Acceptance:

- the backend/runtime contract for standards is explicit
- code, runtime docs, and examples all match that contract
- there is no remaining claim that implies richer runtime semantics unless the
  runtime actually implements them

Tracked slices:

- [ ] D1. Make the runtime crate docs and V2 strategy docs agree on the current
  standards/generics contract.
- [ ] D2. Add explicit tests/docs that standards remain procedural and
  compile-time constrained unless backend/runtime semantics expand further.


# 7. Workstream E: Example Portability And Honest Execution

Goal:

- make the shipped V2 examples runnable in the way the docs describe, without
  hidden harness assumptions

Known audit findings:

- positive V2 examples do run in the main harness
- some examples rely on package-store wiring or test harness setup that is not
  obvious from the docs
- at least one example still carries an old "negative boundary" identity in
  docs even though it is now positive

Required work:

- retag examples whose role changed from negative boundary to positive example
- document exact setup for bundled `std` example execution
- reduce harness-only magic where practical
- add explicit integration coverage for the documented invocation path

Primary files:

- `examples/fail_generic_type_m1/...`
- `examples/generic_type_exec_m1m2/...`
- `examples/generic_standard_constraint_m1m2/...`
- `test/integration_tests/integration_editor_and_build.rs`
- `docs/v2-generics-m1.md`
- `docs/v2-standards-m2.md`
- `docs/bundled-std.md`

Acceptance:

- no positive example is still documented as a negative boundary
- the documented way to run shipped V2 examples is the way they are actually
  tested
- bundled `std` setup is explicit and reproducible

Tracked slices:

- [ ] E1. Retag example inventories and docs where positive V2 examples still
  carry negative-boundary names.
- [ ] E2. Document the exact bundled-`std` execution/setup path used by shipped
  V2 examples.


# 8. Workstream F: Remaining Narrow-Surface Boundaries

Goal:

- keep the narrow shipped V2 contract honest by making unsupported edge cases
  explicit and tested

Known remaining narrow boundaries from the audit:

- generic receiver types are still restricted
- generic error types are still restricted
- underconstrained generics are still rejected
- generic routines are not ordinary routine values
- recursive generic type instantiation is still rejected
- protocol standards still only allow the current narrow required routine
  signature surface

Required work:

- decide which of these remain part of the intentionally narrow shipped V2
  contract
- for every kept boundary:
  - add or repair explicit negative tests
  - document it as a current boundary
- for every boundary the project wants to lift:
  - create a dedicated implementation slice with tests across compiler,
    editor, and docs

Primary files:

- `lang/compiler/fol-typecheck/src/decls.rs`
- `lang/compiler/fol-typecheck/src/exprs/calls.rs`
- milestone docs and example fixtures

Acceptance:

- every remaining unsupported V2 edge case is either:
  - explicitly documented and tested as a current boundary
  - or fully implemented and reflected everywhere

Tracked slices:

- [ ] F1. Audit remaining narrow generic boundaries and pin each one as either
  a documented current limit or a future implementation slice.


# 9. Recommended Order

Work should proceed in this order:

1. contract/doc cleanup so the tracker is honest again
2. tree-sitter parity and missing real test coverage
3. stale editor regression cleanup
4. example relabeling and portable execution path cleanup
5. hard decision on backend/runtime meaning of standards
6. close remaining narrow-boundary documentation/tests

Reason:

- steps 1 through 4 remove false signals and stale regressions
- step 5 is the only item that may require a deeper architecture choice
- step 6 prevents the project from drifting back into ambiguous claims


# 10. Completion Rule

This plan is complete only when:

- compiler, backend/runtime, editor, tree-sitter, tests, examples, and docs
  all describe the same V2 product
- deferred broader-V2 ideas remain clearly deferred
- no current shipped example or test still relies on stale milestone language
- the project can honestly say "V2 is fully implemented" without narrowing
  that claim to "compiler-side only"
