# V2 Full Implementation Plan

Last updated: 2026-04-11

This plan replaces the old production-readiness audit plan.
Its purpose is different:

- define the work needed to make `V2` of FOL real
- keep the book, compiler, backend, editor, examples, and tests aligned
- separate already-landed narrow milestones from the still-missing broader `V2`

This plan assumes:

- `V1` production hardening is already done
- `V2` means more than parser acceptance and semantic boundaries
- a `V2` feature is not complete until it has:
  - parser support
  - resolver support
  - typecheck support
  - lowering support where required
  - backend/runtime support where required
  - editor/tree-sitter audit
  - real example coverage
  - negative example coverage
  - book and docs updates

This plan also assumes the repo should keep the current no-legacy policy:

- no compatibility shims
- no parallel old/new semantics
- no fallback design paths once a direction is chosen

## 1. Current Truth

The current repo state is:

- `V2 Milestone 1` is partially real for a narrow generic-routine subset
- `V2 Milestone 2` is partially real for a narrow protocol-standard subset
- both milestones currently stop before full lowering/backend execution
- broader `V2` design still exists only in docs/book examples or parser surface

The key current contracts are:

- [book/src/500_items/500_generics.md](/home/bresilla/data/code/bresilla/fol/book/src/500_items/500_generics.md)
- [docs/v2-generics-m1.md](/home/bresilla/data/code/bresilla/fol/docs/v2-generics-m1.md)
- [book/src/500_items/400_standards.md](/home/bresilla/data/code/bresilla/fol/book/src/500_items/400_standards.md)
- [docs/v2-standards-m2.md](/home/bresilla/data/code/bresilla/fol/docs/v2-standards-m2.md)

Current explicit non-completion boundaries include:

- generic routine lowering/backend still blocked in [lang/compiler/fol-lower/src/decls/routine_decls.rs](/home/bresilla/data/code/bresilla/fol/lang/compiler/fol-lower/src/decls/routine_decls.rs)
- generic parameter type lowering still blocked in [lang/compiler/fol-lower/src/session.rs](/home/bresilla/data/code/bresilla/fol/lang/compiler/fol-lower/src/session.rs)
- protocol standard lowering/backend still blocked in [lang/compiler/fol-lower/src/session.rs](/home/bresilla/data/code/bresilla/fol/lang/compiler/fol-lower/src/session.rs)
- blueprint and extended standards still intentionally unsupported in [lang/compiler/fol-typecheck/src/decls.rs](/home/bresilla/data/code/bresilla/fol/lang/compiler/fol-typecheck/src/decls.rs)

## 2. Exit Criteria For “V2 Complete”

`V2` should only be called complete when all of the following are true:

- generic routines compile, lower, emit, and run in supported `V2` cases
- generic type declarations and instantiations compile, lower, emit, and run
- chosen generic constraint semantics are implemented, not just documented
- protocol standards compile, lower, emit, and run
- blueprint standards compile, lower, emit, and run if they remain part of `V2`
- extended standards compile, lower, emit, and run if they remain part of `V2`
- standards/generics interaction is implemented for the chosen contract
- editor behavior is updated to reflect real `V2`, not just boundary messaging
- tree-sitter supports the final `V2` syntax surface actually shipped
- the book stops describing major `V2` pieces as future work

If any one of those is false, `V2` is not complete.

## 3. Product Decisions To Freeze First

These decisions should be made before large implementation work starts.
Without them, the compiler will drift into multiple partial semantics.

### 3.1 Freeze The Actual V2 Surface

Decide whether full `V2` includes:

- generic routines only, or generic routines plus generic types
- constraints only as named standards, or a wider constraint language
- protocol standards only, or protocol + blueprint + extended standards
- standards as non-value contracts only, or also as ordinary type surfaces
- procedural conformance only, or any dispatch/inference behavior

Required outputs:

- update [book/src/500_items/500_generics.md](/home/bresilla/data/code/bresilla/fol/book/src/500_items/500_generics.md)
- update [book/src/500_items/400_standards.md](/home/bresilla/data/code/bresilla/fol/book/src/500_items/400_standards.md)
- either retire or rewrite [docs/v2-generics-m1.md](/home/bresilla/data/code/bresilla/fol/docs/v2-generics-m1.md)
- either retire or rewrite [docs/v2-standards-m2.md](/home/bresilla/data/code/bresilla/fol/docs/v2-standards-m2.md)

Acceptance criteria:

- one exact shipped `V2` contract exists in docs
- no milestone note still describes a now-implemented feature as unsupported
- no book chapter presents future syntax as if it already ships

### 3.2 Freeze Runtime/Backend Strategy For V2 (complete, verified 2026-04-11)

Decide whether `V2` generics and standards lower by:

- monomorphization
- dictionary/witness passing
- a hybrid model

This must be decided before lowering work for generics and standards.

Required outputs:

- one backend strategy note in `docs/`
- one emitted-IR strategy implemented in `fol-lower`
- one backend emission strategy implemented in `fol-backend`

Acceptance criteria:

- generics and standards use the same chosen model across lowering and backend
- test expectations describe one model, not several competing ones

## 4. Workstream A: Finish Current Narrow M1 Generics End To End

This is the first execution milestone because the repo already has parser, resolver,
typecheck, examples, and editor depth for the narrow subset.

### 4.1 Lower Generic Routine Declarations

Current gap:

- generic routines typecheck but lowering stops with an explicit boundary

Primary crates:

- `lang/compiler/fol-lower`
- `lang/compiler/fol-typecheck`

Tasks:

- lower generic routine declarations into a backend-owned generic IR shape
- lower generic parameter references in:
  - routine parameter types
  - routine return types
  - nested shell/container positions that the book chooses to support
- reject unsupported generic signatures earlier if they are still out of scope

Tests:

- convert existing lowering-boundary examples into lowering-success examples:
  - `examples/generic_routine_m1`
  - `examples/generic_routine_pair_m1`
  - `examples/generic_routine_cross_file_m1`
- add IR snapshot coverage for:
  - single generic routine
  - cross-file generic routine
  - imported generic routine call
  - nested generic type positions allowed by contract

Acceptance criteria:

- `--dump-lowered` succeeds on all positive M1 examples
- no generic-routine lowering boundary remains in `fol-lower`

### 4.2 Emit And Execute Generic Routine Calls

Primary crates:

- `lang/execution/fol-backend`
- `lang/execution/fol-runtime`
- `lang/tooling/fol-frontend`

Tasks:

- implement backend emission for chosen generic lowering strategy
- ensure generated Rust/runtime support matches `core`/`memo` model legality
- keep generated symbol naming stable and debuggable
- make generic calls work across files and packages

Tests:

- positive app/example compile-and-run tests
- emitted-Rust snapshot tests
- cross-package integration tests

Acceptance criteria:

- positive M1 examples compile and execute
- no backend/runtime panic or placeholder path exists for supported generics

### 4.3 Expand M1 Semantics To The Chosen Narrow Edge Cases

Tasks:

- decide supported status for receiver-qualified generic routines
- decide supported status for default arguments in generic routines
- decide supported status for recoverable/generic interaction
- implement or reject each path explicitly and early

Tests:

- parser, resolver, typecheck, lowering, and runtime coverage for each chosen case
- negative tests for every non-chosen case

Acceptance criteria:

- no generic edge case is parser-only or typecheck-only by accident

## 5. Workstream B: Implement Generic Types

This is the biggest missing semantic block if full `V2` includes generics beyond M1.

### 5.1 Resolve The Generic Type Contract (complete, verified 2026-04-11)

Tasks:

- define which type forms can be generic:
  - record types
  - entry types
  - aliases
  - containers with generic arguments
- define instantiation syntax and canonical spelling
- define where generic arguments are inferred vs required

Required docs:

- rewrite the generic type sections in [book/src/500_items/500_generics.md](/home/bresilla/data/code/bresilla/fol/book/src/500_items/500_generics.md)

Acceptance criteria:

- one exact syntax and meaning for generic types exists in the book

### 5.2 Parser And Resolver Support For Generic Types

Primary crates:

- `lang/compiler/fol-parser`
- `lang/compiler/fol-resolver`

Tasks:

- accept generic type declarations in final shipped syntax
- bind generic type parameters in the correct scopes
- resolve instantiated generic type references across files/packages

Tests:

- parser inventory for type generics
- resolver scope tests for generic type parameter visibility
- imported/cross-file generic type reference tests

Acceptance criteria:

- generic type syntax is parse/resolve complete for the chosen contract

### 5.3 Typecheck Generic Types

Primary crate:

- `lang/compiler/fol-typecheck`

Tasks:

- represent generic type declarations and instantiated types in typed IR
- validate generic argument counts and compatibility
- support generic fields, nested uses, and aliasing where chosen
- reject unsupported recursive or unsized cases explicitly

Tests:

- positive examples for instantiated generic records/aliases
- negative examples for arity mismatch, invalid constraints, unsupported recursion

Acceptance criteria:

- generic type declarations and uses are fully typed without fallback wording

### 5.4 Lower And Emit Generic Types

Primary crates:

- `lang/compiler/fol-lower`
- `lang/execution/fol-backend`

Tasks:

- lower instantiated generic types into backend-owned lowered types
- emit stable runtime/backend representations
- ensure equality, defaulting, field access, method sugar, and shell wrapping all work

Tests:

- emitted-Rust snapshots for instantiated types
- compile-and-run app fixtures using instantiated generic records and aliases

Acceptance criteria:

- generic types are executable, not just semantic

## 6. Workstream C: Implement Constraints And Generic/Standards Interaction

The book currently keeps most of this outside M1/M2.
If “full V2” is the goal, it must be made explicit and real.

### 6.1 Define Constraint Surface (complete, verified 2026-04-11)

Decide whether generic constraints are:

- standards-only
- a broader trait-like surface
- limited to protocol standards

Tasks:

- define syntax and meaning
- define where constraint solving is allowed
- define diagnostics for unsatisfied constraints

Acceptance criteria:

- current negative seam `fail_generic_standard_constraint_m1m2` becomes either:
  - a positive example if implemented
  - a moved-out-of-V2 example if deferred

### 6.2 Implement Constraint Resolution And Checking

Primary crates:

- `fol-resolver`
- `fol-typecheck`

Tasks:

- resolve constraint references
- bind constrained generic parameters
- check conformance at call sites and instantiation sites
- produce precise diagnostics for failed constraint satisfaction

Tests:

- positive constrained generic examples
- negative examples for missing conformance and ambiguous matches

Acceptance criteria:

- constraints are real semantics, not parser sugar or boundary messages

### 6.3 Lower And Emit Constrained Generics

Tasks:

- pass evidence/witnesses or monomorphize by constrained concrete instantiation
- ensure backend model matches the chosen generics strategy

Acceptance criteria:

- constrained generic examples compile and run

## 7. Workstream D: Finish Current Narrow M2 Standards End To End

This is the second execution milestone already close semantically.

### 7.1 Lower Protocol Standards

Current gap:

- protocol standards typecheck but lowering stops explicitly

Primary crates:

- `fol-lower`
- `fol-typecheck`

Tasks:

- lower protocol standard declarations into lowered contract metadata
- lower type-side conformance claims
- lower exact required-routine matches and extra-overload acceptance

Tests:

- convert current lowering-boundary examples into lowering-success examples:
  - `examples/standards_protocol_m2`
  - `examples/standards_protocol_pair_m2`
  - `examples/standards_protocol_multi_m2`
- add lowered snapshots for:
  - single protocol
  - multi-protocol conformance
  - imported protocol conformance

Acceptance criteria:

- `--dump-lowered` succeeds on positive protocol-standard examples

### 7.2 Emit And Execute Protocol-Conforming Programs

Primary crates:

- `fol-backend`
- `fol-runtime`

Tasks:

- emit the chosen runtime/backend representation of standards and conformance
- ensure receiver-qualified routine calls through standards work under the chosen model
- keep semantics procedural, not accidental object dispatch, unless the contract changes

Tests:

- compile-and-run tests for positive standards M2 examples
- emitted-Rust tests for conformance metadata plumbing

Acceptance criteria:

- protocol-standard examples compile and run end to end
- no explicit M2 lowering/backend boundary remains for protocol standards

## 8. Workstream E: Implement Blueprint Standards

This work only starts if blueprint standards remain inside full `V2`.

### 8.1 Freeze Blueprint Meaning (complete, verified 2026-04-11)

Tasks:

- define whether blueprints describe required data members only
- define how type-side claims check blueprint requirements
- define interaction with aliases, entries, optional/error shells, and generics

Acceptance criteria:

- blueprint semantics are documented in the book and not still labeled future

### 8.2 Parser/Resolver/Typecheck/Lower/Backend

Tasks:

- keep parser syntax if already chosen, otherwise simplify it
- resolve blueprint declarations and claims
- check required data-member conformance
- lower and emit the chosen contract representation

Tests:

- positive blueprint examples
- negative examples for missing fields, type mismatches, ambiguous members

Acceptance criteria:

- blueprint examples compile and run or compile and statically validate if runtime execution is irrelevant

## 9. Workstream F: Implement Extended Standards

This work only starts if extended standards remain inside full `V2`.

### 9.1 Freeze Extended Meaning (complete, verified 2026-04-11)

Tasks:

- define the exact combination of routine and data requirements
- define conflict resolution between protocol/blueprint/extended claims

### 9.2 Implement Extended Standards Across The Full Pipeline

Tasks:

- resolver collection
- typecheck conformance
- lowering
- backend emission
- editor/tree-sitter support

Tests:

- positive extended examples
- negative examples for partial conformance and conflict cases

Acceptance criteria:

- extended standards are no longer parser-only or doc-only

## 10. Workstream G: Decide And Implement Dispatch Rules

This is the largest semantic decision still floating in the book examples.

Questions the codebase must answer before implementation:

- are standards used only for conformance checking, or also for dispatch?
- are generic constraints callable only by monomorphized concrete routines?
- is `value.bar()` through constrained generics allowed in `V2`?
- is this still procedural call binding, or a new dispatch layer?

Tasks:

- freeze one dispatch model in the book
- implement resolver/typecheck call selection accordingly
- implement lowering/backend accordingly

Acceptance criteria:

- no example in the book implies a dispatch model the compiler does not implement

## 11. Workstream H: Editor, LSP, And Tree-Sitter For Real V2

The current repo explicitly says editor support is not broadly V2-aware.
That must change if `V2` is going to be called complete.

### 11.1 LSP Semantic Awareness

Tasks:

- hover and definition for generic declarations, instantiations, standards, conformance claims
- completion for:
  - generic parameters in valid scopes
  - generic type instantiations if shipped
  - standards/conformance-related names if shipped
- diagnostics and code actions that reflect real V2 semantics

Tests:

- real example-root editor tests for every shipped V2 example

Acceptance criteria:

- `fol-editor` no longer documents “no V2-aware editor behavior”

### 11.2 Tree-Sitter Grammar And Queries

Tasks:

- ensure final shipped V2 syntax is represented in:
  - `grammar.js`
  - highlights queries
  - locals queries
  - symbols queries
- remove captures for non-shipped V2 syntax if the syntax is dropped

Tests:

- real example query tests for all positive and negative V2 examples

Acceptance criteria:

- tree-sitter matches shipped V2 syntax, not speculative future syntax

## 12. Workstream I: Examples, Fixtures, And Book Contract Cleanup

The example set must become the public truth table for V2.

### 12.1 Replace Boundary Examples With Execution Examples Where Appropriate

Current positive V2 examples mostly prove “open/typecheck then stop at lowering”.
That is not enough for a complete V2.

Tasks:

- upgrade positive M1/M2 examples from boundary fixtures to execution fixtures
- add richer examples for:
  - generic types
  - constrained generics
  - blueprint/extended standards if included

Acceptance criteria:

- positive examples compile and run
- negative examples fail with exact intentional diagnostics

### 12.2 Remove Or Retag No-Longer-Correct Milestone Notes

Tasks:

- remove milestone wording once the milestone is fully shipped
- keep a changelog/history note elsewhere if needed

Acceptance criteria:

- docs do not simultaneously call a feature “implemented” and “still unsupported”

## 13. Workstream J: Cross-Cutting Quality Gates

Every V2 slice should satisfy all of these before merge.

### 13.1 Compiler Pipeline Gate

- parser test
- resolver test
- typecheck test
- lowering test
- backend/emitted-Rust test
- app/example compile-and-run test where relevant

### 13.2 Tooling Gate

- editor-opened example test
- hover/definition test for new declarations
- tree-sitter query test

### 13.3 Contract Gate

- book chapter updated
- docs note updated or deleted
- example matrix updated
- negative examples updated

## 14. Recommended Delivery Order

This is the suggested implementation order.

### Phase 1

- freeze full `V2` product contract
- freeze generic/standard backend strategy

### Phase 2

- finish executable M1 generic routines end to end

### Phase 3

- finish executable M2 protocol standards end to end

### Phase 4

- implement generic types

### Phase 5

- implement constraints and generic/standards interaction

### Phase 6

- implement blueprint standards if still inside `V2`

### Phase 7

- implement extended standards if still inside `V2`

### Phase 8

- implement chosen dispatch/inference semantics if still inside `V2`

### Phase 9

- finish V2-aware editor/tree-sitter/LSP
- rewrite book/docs from milestone language to shipped-contract language

## 15. Concrete Backlog

The first concrete backlog after this plan replacement should be:

1. [complete, verified 2026-04-11] write one `docs/v2-full-contract.md` file that freezes what “full V2” means
2. implement generic routine lowering in `fol-lower`
3. implement backend emission for positive generic routine examples
4. convert generic M1 examples from lowering-boundary tests into run tests
5. implement protocol-standard lowering in `fol-lower`
6. implement backend emission for positive standards M2 examples
7. convert standards M2 examples from lowering-boundary tests into run tests
8. decide whether generic types, blueprint standards, extended standards, and standards-as-constraints are in or out for full `V2`
9. if in, implement them in that order with full pipeline and editor coverage

## 16. Open Question (complete, resolved 2026-04-11)

Resolved decision:

- full `V2` in this repo includes only:
  - executable generic routines
  - executable protocol standards
  - generic types
  - standards-as-constraints
- broader dispatch/inference work shown in future-facing book examples remains
  outside this `V2` target

Implementation rule after this decision:

- do not widen `V2` to standards-driven dispatch
- do not widen `V2` to broader inference semantics
- keep blueprint and extended standards outside the full `V2` target unless the
  contract is changed explicitly later
