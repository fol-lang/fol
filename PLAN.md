# V2 Hardening Plan: Second Pass

This file is the next hardening plan for the current landed `V2` subset after
the first broad hardening pass.

It is based on:

- a fresh local repo sweep
- one independent generics scan
- one independent standards scan
- one independent tooling/cross-layer scan

The three scans converged on the same pattern:

- parser and primary typecheck support are no longer the biggest gap
- the remaining risk is mostly in edge-shape semantics
- cross-file and imported cases are still lighter than local single-file cases
- editor and tree-sitter coverage is still shallower than compiler coverage
- the public example and docs matrix is honest, but still too manual

This plan is not for new `V2` features.
It is for deeper hardening of the already landed subset:

- Milestone 1
  - generic routine core
- Milestone 2
  - protocol standards and procedural conformance core

Success criteria:

- generic edge shapes are pinned more deeply
- standards conformance is hardened across imported and multi-standard cases
- checked-in example packages cover more positive and negative cases
- LSP/tree-sitter coverage exercises real V2 examples more deeply
- V2 inventories and docs become harder to let drift
- `make build` passes
- `make test` passes

## Epoch 1: Freeze The Second-Pass Scope

### Slice 1 (complete)
Write one second-pass hardening note for Milestone 1 generics in:

- `docs/v2-generics-m1.md`

The note must explicitly call out these remaining gaps as current targets:

- receiver-qualified generic routines
- richer signature-position use of generic parameters
- imported/cross-file generic routine calls
- editor/tree-sitter coverage depth

Completion criteria:

- the doc names the second-pass hardening targets explicitly
- it does not pretend these targets are already fully covered

### Slice 2 (complete)
Write one second-pass hardening note for Milestone 2 standards in:

- `docs/v2-standards-m2.md`

The note must explicitly call out these remaining gaps as current targets:

- multi-standard conformance on one type
- imported-standard conformance typecheck truth
- unsupported requirement-shape diagnostics
- deeper editor/tree-sitter coverage

Completion criteria:

- the doc names the second-pass standards targets explicitly
- it keeps blueprint and extended standards out of scope

### Slice 3 (complete)
Add one top-level inventory regression that pins this second-pass plan to the
current known V2 example set and milestone docs.

Completion criteria:

- there is one regression that fails if the V2 example/doc matrix drifts
- it covers both M1 and M2 example groups

## Epoch 2: Generics Signature And Scope Edge Cases

### Slice 4 (complete)
Add parser truth tests for receiver-qualified generic routine declarations.

Target cases:

- receiver-qualified generic header with one generic parameter
- receiver-qualified generic header with two generic parameters
- mixed receiver plus default/named parameter syntax where parser allows it

Completion criteria:

- parser truth is pinned for receiver-qualified generic headers

### Slice 5 (complete)
Add typecheck boundary tests for receiver-qualified generic routines.

The tests must make the current Milestone 1 contract explicit:

- either accepted at the same M1 semantic tier as plain generic routines
- or rejected with one exact M1 boundary message

Completion criteria:

- receiver-qualified generic routine semantics are no longer implicit
- the exact current contract is pinned by tests

### Slice 6 (complete)
Add resolver tests for generic parameter leakage into richer signature
positions.

Target cases:

- nested record field annotations
- alias-backed signature annotations
- nested optional or error shells
- nested container annotations

Completion criteria:

- generic scope leakage is pinned beyond the current simple parameter/return
  cases

### Slice 7 (complete)
Add typecheck tests for generic routines using generic parameters in richer
signature shapes.

Target cases:

- `opt[T]`
- `err[T]`
- `seq[T]`
- `vec[T]`
- nested array or record positions if legal

Completion criteria:

- M1 behavior for these signature shapes is explicit
- accepted cases and rejected cases are both pinned

### Slice 8 (complete)
Add negative tests for generic routines mixed with defaulted and variadic
parameter interactions where inference becomes unclear.

Completion criteria:

- default/variadic/generic interactions are no longer only parser-hard
- the semantic boundary is explicit

## Epoch 3: Imported And Cross-File Generics Hardening

### Slice 9 (complete)
Add resolver and typecheck tests for imported generic routine calls through
`loc` package boundaries.

Target cases:

- imported direct identity-style generic call
- imported two-parameter generic call
- imported underconstrained generic call

Completion criteria:

- imported generic routine calls are pinned independently of local-file cases

### Slice 10 (complete)
Add one checked-in positive cross-file generic example package.

Suggested shape:

- generic routine declared in one file or package
- imported and called from another file
- editor can open both files cleanly
- lower still stops with the exact M1 boundary

Completion criteria:

- one positive cross-file generic example exists
- it is wired into integration coverage

### Slice 11
Add one checked-in negative cross-file generic example package.

Suggested shape:

- imported generic routine used in an underconstrained or first-class misuse way

Completion criteria:

- one negative cross-file generic example exists
- CLI and editor diagnostics both pin the failure cleanly

### Slice 12
Add lowering regression coverage for cross-file generic examples.

Completion criteria:

- lowering-stop behavior is pinned for imported/cross-file generic routines
- the boundary message remains exact and consistent

## Epoch 4: Standards Conformance Edge Cases

### Slice 13
Add typecheck tests for multi-standard conformance on one type.

Target cases:

- one type conforming to two protocol standards
- one satisfied and one missing
- both satisfied with separate receiver-qualified routines

Completion criteria:

- multi-standard conformance is no longer only resolver-hard

### Slice 14
Add typecheck tests for imported-standard conformance.

Target cases:

- protocol standard declared in one file/package
- conformance claimed in another file/package
- positive and negative forms

Completion criteria:

- imported-standard conformance is pinned at the typecheck layer

### Slice 15
Add standards typecheck tests for unsupported required-routine shapes with one
test per message family.

Target shapes:

- generic required routine
- receiver-qualified required routine
- capturing required routine
- default implementation in protocol standard

Completion criteria:

- each unsupported requirement shape has its own exact diagnostic coverage

### Slice 16
Add standards typecheck tests for ambiguous matching routines in broader
realistic shapes.

Completion criteria:

- ambiguity diagnostics are pinned beyond the current small cases

### Slice 17
Add standards typecheck tests for multi-file conformance where the same
receiver has extra unrelated routines and overloads.

Completion criteria:

- exact-match acceptance and mismatch rejection are hardened in multi-file
  settings

## Epoch 5: Standards Examples And Lowering Hardening

### Slice 18
Add one checked-in positive multi-standard example package.

Suggested shape:

- one type
- two protocol standards
- both conformance claims present
- editor can open all files
- lowering stops with the exact M2 boundary

Completion criteria:

- one positive multi-standard example exists and is integrated

### Slice 19
Add one checked-in negative standards example for missing required routine.

Completion criteria:

- the example fails with the exact missing-routine conformance diagnostic

### Slice 20
Add one checked-in negative standards example for incompatible routine
signature.

Completion criteria:

- the example fails with the exact signature-mismatch conformance diagnostic

### Slice 21
Add one checked-in negative standards example for imported-standard ambiguity
or imported-standard unsupported claim if that is the real current boundary.

Completion criteria:

- imported standards edge behavior is represented by a real example package

### Slice 22
Expand lowering and CLI full-chain coverage to all new standards examples.

Completion criteria:

- lowering-stop behavior is pinned across all new positive and negative
  standards examples

## Epoch 6: Cross-Feature Seam Between M1 And M2

### Slice 23
Add one negative example package that mixes generic routines with standards in
an explicitly unsupported way.

Suggested shape:

- generic routine constrained by a protocol standard

Completion criteria:

- the seam between M1 generics and M2 standards is represented by a real
  checked-in example

### Slice 24
Add resolver and typecheck tests for generic constraints trying to reference
standards.

Completion criteria:

- the existing boundary is pinned beyond the current narrow coverage

### Slice 25
Add CLI and lowering full-chain tests for the mixed generic-plus-standards
negative seam.

Completion criteria:

- the mixed seam fails consistently in the full path

### Slice 26
Add editor diagnostics coverage for the mixed generic-plus-standards negative
example.

Completion criteria:

- editor mirrors the current compiler-owned seam honestly

## Epoch 7: V2 Editor Navigation And Completion Hardening

### Slice 27
Add real-example hover coverage for V2 generics examples.

Target examples:

- `examples/generic_routine_m1`
- `examples/generic_routine_pair_m1`

Target assertions:

- generic routine names resolve
- hover text does not overclaim lowering/backend support

Completion criteria:

- hover is pinned on real generics examples

### Slice 28
Add real-example definition coverage for V2 generics examples.

Target assertions:

- call-site to declaration navigation works on checked-in generic examples

Completion criteria:

- definition is pinned on real generics examples

### Slice 29
Add real-example hover and definition coverage for standards examples.

Target examples:

- `examples/standards_protocol_m2`
- `examples/standards_protocol_pair_m2`

Target assertions:

- type contract headers resolve to standard declarations
- required routine declarations resolve cleanly

Completion criteria:

- standards navigation is pinned on real examples

### Slice 30
Add completion coverage for current V2-safe contexts.

Target cases:

- completion near generic call sites
- completion in standards bodies
- completion around type-side contract headers
- completion on negative V2 examples without fake smartness

Completion criteria:

- V2 completion is no longer only indirectly covered through runtime/std cases

### Slice 31
Add semantic-token assertions for one generics example and one standards
example.

Target token categories:

- `std`
- `pro`
- generic parameter positions
- type contract header positions

Completion criteria:

- semantic-token coverage is more specific for V2 syntax

## Epoch 8: Tree-Sitter Locals And Symbols Hardening

### Slice 32
Add tree-sitter locals coverage for one positive generics example.

Suggested target:

- `examples/generic_routine_pair_m1`

Completion criteria:

- generic declarations and local names are exercised beyond highlights

### Slice 33
Add tree-sitter symbols coverage for one positive standards example.

Suggested target:

- `examples/standards_protocol_pair_m2`

Completion criteria:

- protocol declarations and type-side conformance syntax are exercised in
  symbols coverage

### Slice 34
Add one negative syntax-oriented V2 example to tree-sitter locals/symbols
coverage.

Completion criteria:

- locals/symbol extraction stays sane on malformed V2-oriented syntax

### Slice 35
Add a tree-sitter coverage note or test that keeps highlight-only V2 audits
from being mistaken for full locals/symbol coverage.

Completion criteria:

- tree-sitter coverage boundaries are explicit in code or docs

## Epoch 9: Inventory And Docs Sync Hardening

### Slice 36
Add one repo-scan regression for the V2 example inventory by naming convention
or explicit marker.

Completion criteria:

- adding a new V2 example without inventory updates becomes visible

### Slice 37
Add one repo-scan regression that checks milestone docs and book chapters for
the current shipped V2 example matrix.

Completion criteria:

- docs/book drift against real checked-in V2 examples is harder to introduce

### Slice 38
Update:

- `docs/v2-generics-m1.md`
- `docs/v2-standards-m2.md`

so they list the expanded current example matrix and the newly hardened
boundaries.

Completion criteria:

- milestone docs reflect the new examples and new exact boundaries

### Slice 39
Update:

- `book/src/500_items/500_generics.md`
- `book/src/500_items/400_standards.md`

to reflect the more exact current subset and examples without widening the
public contract.

Completion criteria:

- book wording is more exact about current V2 hardening state

## Epoch 10: Final Cross-Layer Closure

### Slice 40
Run and harden targeted parser/resolver/typecheck suites for the new generics
cases.

Completion criteria:

- targeted generics suites are green
- no new brittle failures remain

### Slice 41
Run and harden targeted parser/resolver/typecheck suites for the new standards
cases.

Completion criteria:

- targeted standards suites are green
- no new brittle failures remain

### Slice 42
Run and harden targeted editor/tree-sitter suites for the new V2 example
coverage.

Completion criteria:

- V2 editor/tree-sitter suites are green
- example-driven coverage is stable

### Slice 43
Run the required full gate:

- `make build`
- `make test`

Completion criteria:

- both commands pass

### Slice 44
Update:

- `plan/VERSIONS.md`
- `AGENTS.md`

if needed so contributor guidance reflects the deeper hardened V2 example and
tooling obligations.

Completion criteria:

- contributor/version docs stay aligned with the hardened subset

### Slice 45
Do one final repo sweep for stale V2 claims that overstate:

- generic routine support
- standards support
- editor V2 support
- tree-sitter V2 coverage

Completion criteria:

- stale overclaims are removed
- the second-pass hardening plan can be marked complete
