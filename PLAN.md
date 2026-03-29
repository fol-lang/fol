# V2 Hardening Plan

This file is the detailed hardening plan for the already-landed `V2` work:

- Milestone 1
  - generic routine core
- Milestone 2
  - protocol standards and procedural conformance core

This is not a feature-expansion plan.
It is not the next abstraction milestone.
It is a stability and honesty plan.

The goal is:

- make the current `V2` subsets much harder to accidentally regress
- add many more positive and negative tests
- add many more real example packages
- pin edge cases across parser, resolver, typecheck, lowering, CLI, editor,
  and docs

Success criteria:

- the current landed `V2` subset is described more precisely than before
- edge-case misuse is rejected explicitly and consistently
- positive examples and negative examples both expand materially
- editor and tree-sitter coverage reflect the real shipped subset
- repo docs/book stop hand-waving over current boundaries
- `make build` passes
- `make test` passes

## Epoch 1: Freeze The Hardening Contract

### Slice 1 (complete)
Write one hardening note for Milestone 1 generics that states:

- what is implemented
- what is intentionally unsupported
- what must fail in parser
- what must fail in resolver/typecheck
- what must fail in lowering

Completion criteria:

- one exact generics hardening note exists in repo docs
- it names positive and negative obligations explicitly

### Slice 2 (complete)
Write one hardening note for Milestone 2 standards that states:

- what is implemented
- what is intentionally unsupported
- what must fail in resolver/typecheck
- what must fail in lowering
- what the editor should and should not promise

Completion criteria:

- one exact standards hardening note exists in repo docs
- it distinguishes semantic support from parser-only acceptance

### Slice 3 (complete)
Add one top-level regression inventory test that pins the current public `V2`
subset as:

- generic routines only for Milestone 1
- protocol standards only for Milestone 2

Completion criteria:

- one current-subset inventory test exists
- future accidental widening becomes visible immediately

## Epoch 2: Parser Hardening For Generics

### Slice 4 (complete)
Add parser truth tests for multiple generic routine header forms that are still
accepted in Milestone 1.

Completion criteria:

- generic routine parser inventory is materially broader than today

### Slice 5 (complete)
Add parser negative tests for malformed generic parameter lists.

Target cases:

- empty names
- repeated separators
- trailing malformed tokens
- broken nested header punctuation

Completion criteria:

- malformed generic headers fail with explicit parser coverage

### Slice 6 (complete)
Add parser tests for generic routines mixed with:

- captures
- default parameters
- variadics
- named parameters

Completion criteria:

- parser truth is pinned for these mixed surfaces even when semantics are
  narrower

### Slice 7 (complete)
Add parser negative tests for template-call-like or explicit generic-call
surfaces that Milestone 1 does not support.

Completion criteria:

- parser or later boundary for unsupported explicit generic-call syntax is
  pinned clearly

## Epoch 3: Resolver And Scope Hardening For Generics

### Slice 8 (complete)
Add resolver tests for nested-scope generic parameter visibility.

Target cases:

- nested routine body references
- shadowing by locals
- shadowing by parameters
- sibling routine non-visibility

Completion criteria:

- generic scope rules are pinned beyond the current simple cases

### Slice 9 (complete)
Add resolver tests for duplicate generic parameter names across:

- direct duplicates
- duplicates against routine parameters
- duplicates against captures where relevant

Completion criteria:

- ownership of duplicate-name diagnostics is explicit and tested

### Slice 10 (complete)
Add resolver tests for generic parameter misuse in out-of-scope type
positions.

Completion criteria:

- generic parameter leakage beyond routine-local scope fails clearly

## Epoch 4: Typecheck Hardening For Generics

### Slice 11
Add more positive typecheck tests for direct generic routine calls.

Target cases:

- one parameter identity
- two parameters with same inferred type
- mixed scalar families
- nested call use in expressions

Completion criteria:

- positive Milestone 1 inference coverage is materially broader

### Slice 12
Add negative typecheck tests for repeated generic-parameter mismatch shapes.

Target cases:

- int vs str
- array vs scalar
- memo container vs scalar
- alias-backed mismatch

Completion criteria:

- mismatch diagnostics are pinned across more type families

### Slice 13
Add negative typecheck tests for underconstrained generic returns and generic
arguments omitted from inference.

Completion criteria:

- underconstrained-call diagnostics are pinned across multiple forms

### Slice 14
Add negative tests for first-class generic routine misuse.

Target cases:

- binding a generic routine
- passing a generic routine as an argument
- returning a generic routine
- storing a generic routine in aggregates if syntax permits

Completion criteria:

- first-class generic-value rejection is hardened comprehensively

### Slice 15
Add negative tests for generic routines mixed with still-unsupported generic
constraints and generic error shells.

Completion criteria:

- Milestone 1 unsupported combinations are pinned more exhaustively

## Epoch 5: Lowering And CLI Hardening For Generics

### Slice 16
Audit all current generic routine lowering-stop messages and normalize them to
one exact Milestone 1 boundary story.

Completion criteria:

- generic lowering errors are consistent across direct lower and full CLI paths

### Slice 17
Add compile/lowering regression tests for more generic routine shapes that must
stop before lowering.

Target cases:

- multi-parameter generic routines
- generic return-only routines
- generic routines with named/default parameters

Completion criteria:

- explicit lowering boundary is exercised across more realistic generic shapes

### Slice 18
Add a second canonical positive example package for generic routines that:

- opens cleanly in editor
- typechecks cleanly
- still stops at lowering with the exact Milestone 1 boundary

Completion criteria:

- generic examples are not limited to one tiny identity fixture

### Slice 19
Add one canonical negative generic example package for misuse.

Target cases:

- generic type declaration
- generic constraint
- explicit generic-call syntax

Completion criteria:

- one checked-in negative generic package exists and is exercised in integration

## Epoch 6: Parser And Resolver Hardening For Standards

### Slice 20
Add parser inventory tests for more protocol-standard routine signature forms.

Target cases:

- multiple required routines
- multi-parameter signatures
- explicit return/error forms if accepted

Completion criteria:

- parser truth for protocol standards is materially broader

### Slice 21
Add parser negative tests for malformed standard bodies beyond duplicate
members.

Target cases:

- broken separators
- malformed headers
- mixed unsupported member bodies
- malformed routine signature punctuation

Completion criteria:

- malformed standard-body parsing is pinned more deeply

### Slice 22
Add resolver tests for multiple standard declarations and conformance claims in
one source unit and across files.

Completion criteria:

- standard symbol resolution is hardened beyond one local file case

### Slice 23
Add resolver tests for conformance claims against:

- imported standards
- shadowed names
- mismatched case
- ambiguous plain names if reachable

Completion criteria:

- conformance name-resolution edge cases are pinned

## Epoch 7: Typecheck Hardening For Standards

### Slice 24
Add more positive conformance tests with multiple required routines.

Completion criteria:

- positive conformance is not pinned only for the one-routine case

### Slice 25
Add negative conformance tests where one of several required routines is
missing.

Completion criteria:

- partial conformance failure stays explicit

### Slice 26
Add negative conformance tests where one required routine matches and another
has an incompatible signature.

Completion criteria:

- mixed missing-vs-mismatch behavior is pinned clearly

### Slice 27
Add negative tests for ambiguous exact matches with overload sets.

Completion criteria:

- ambiguity behavior is pinned across more than one shape

### Slice 28
Add tests for standards mixed with:

- aliases
- records from imported packages
- memo-only types inside otherwise legal procedural conformance

Completion criteria:

- standards subset is exercised under more realistic type environments

### Slice 29
Add negative tests for standards used in ordinary type positions across more
contexts.

Target cases:

- parameters
- returns
- local annotations
- record fields

Completion criteria:

- standards-as-types rejection is pinned comprehensively

## Epoch 8: Lowering, Backend, And CLI Hardening For Standards

### Slice 30
Normalize all standards-lowering boundary diagnostics to one exact Milestone 2
message family.

Completion criteria:

- direct lower and CLI full-chain wording is consistent

### Slice 31
Add lowering regression tests for more positive standards examples that must
stop before backend.

Target cases:

- one protocol / one conforming type
- multiple required routines
- cross-file standard/type definitions

Completion criteria:

- explicit lowering stop is hardened across more real shapes

### Slice 32
Add one additional canonical positive standards example package.

Target cases:

- multi-routine protocol
- cross-file or helper-routine shape

Completion criteria:

- standards examples are broader than a single minimal package

### Slice 33
Add one additional canonical negative standards example package.

Target cases:

- ambiguous routine satisfaction
- standards used as ordinary types
- non-protocol claim

Completion criteria:

- checked-in negative standards coverage is materially broader

## Epoch 9: Editor And Tree-Sitter Hardening

### Slice 34
Add `fol-editor` completion/hover/document-symbol coverage for generic routine
examples.

Completion criteria:

- current editor support for Milestone 1 generics is pinned beyond “opens
  cleanly”

### Slice 35
Add `fol-editor` negative diagnostic coverage for generic misuse.

Target cases:

- generic types
- generic constraints
- explicit generic-call misuse

Completion criteria:

- editor diagnostics stay aligned with compiler-backed generic boundaries

### Slice 36
Add `fol-editor` completion/hover/document-symbol coverage for standards
examples.

Completion criteria:

- current editor support for Milestone 2 standards is pinned beyond one smoke
  case

### Slice 37
Add `fol-editor` negative diagnostic coverage for standards misuse.

Target cases:

- missing required routine
- unsupported blueprint/extended claims
- standards as ordinary types

Completion criteria:

- editor diagnostics stay aligned with compiler-backed standards boundaries

### Slice 38
Audit tree-sitter highlight/locals/symbol behavior for generic and standards
examples and add explicit regression coverage.

Completion criteria:

- syntax-oriented editor assets are explicitly verified against current V2
  examples

## Epoch 10: Docs, Examples Matrix, And Closure

### Slice 39
Expand the checked-in example inventory docs so they mention the real generic
and standards examples now shipped.

Completion criteria:

- docs reference the actual current V2 example set

### Slice 40
Add one top-level shipped-surface matrix test for Milestone 1 examples.

Completion criteria:

- generic examples and boundaries are centrally inventoried

### Slice 41
Add one top-level shipped-surface matrix test for Milestone 2 examples.

Completion criteria:

- standards examples and boundaries are centrally inventoried

### Slice 42
Update book and milestone notes so the hardening work is reflected honestly.

Completion criteria:

- docs describe the stronger example/test surface without overstating features

### Slice 43
Run targeted generic and standards suites and confirm green hardening state.

Completion criteria:

- targeted M1 and M2 suites pass

### Slice 44
Run repo gate.

Completion criteria:

- `make build` passes
- `make test` passes

### Slice 45
Commit the hardening milestone and mark this plan complete.

Completion criteria:

- committed with one conventional title only
- `PLAN.md` fully marked complete
- worktree left clean except unrelated user-owned changes
