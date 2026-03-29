# V2 Milestone 2 Plan

This file is the detailed execution plan for:

- `V2`
- Group 2
- Standards And Conformance

It follows the completed generic-routine core milestone.

It is intentionally narrower than the whole remaining `V2` design.
It is not the full dispatch/inference plan.
It is not blueprint/extension ergonomics beyond what is required to make
contracts real and compiler-backed.

The milestone goal is:

- make one honest contract/conformance core real end to end

That means:

- syntax is accepted deliberately
- resolver owns standard declarations as real symbols
- typecheck can represent standards as contracts
- conformance is checked explicitly
- unsupported later `V2` surfaces fail locally
- editor/tree-sitter are audited
- docs/examples stop pretending broader standards work already exists

The intended Milestone 2 subset is:

- named `std` declarations
- one exact supported standard kind first, then the other chosen kinds
- required receiver-qualified routine signatures
- required data members if the chosen subset includes them
- type declarations that claim conformance
- explicit diagnostics for missing required members

Out of scope for this milestone unless explicitly chosen later in the plan:

- rich generic constraints on standards
- advanced dispatch through standards
- object-style method dispatch
- blanket inference from standards
- full extension machinery beyond the minimal chosen contract subset

Success criteria:

- standards are either enforced end to end or rejected explicitly
- no parser-only “contract” surfaces are left ambiguously half working
- `make build` passes
- `make test` passes

## Epoch 1: Freeze The Contract Milestone

### Slice 1 (complete)
Audit the book/version wording for standards, blueprints, and extensions and
write one exact repo-local Milestone 2 scope note.

Completion criteria:

- one exact repo-local statement exists for what this milestone includes
- it explicitly excludes broader dispatch and generic-constraint work

### Slice 2 (complete)
Freeze the current pre-implementation truth for standards/conformance in tests.

Completion criteria:

- tests prove standards/blueprints/extensions are not silently supported yet
- future semantic deltas become visible

### Slice 3 (complete)
Audit parser coverage for the currently accepted `std` declaration surfaces.

Completion criteria:

- there is an explicit parser truth inventory for:
  - `std name: pro = { ... }`
  - `std name: blu = { ... }`
  - `std name: ext = { ... }`
  - type declarations that claim contracts

### Slice 4 (complete)
Audit resolver/typecheck/lower/editor assumptions that currently reject or
ignore standards surfaces.

Completion criteria:

- known baseline blockers are pinned before semantic work starts

## Epoch 2: Choose The Exact Contract Core Shape

### Slice 5 (complete)
Freeze the first supported standard kind for Milestone 2.

Target behavior:

- either `pro` first
- or one deliberate wider subset

Completion criteria:

- one canonical first contract form is documented in tests/docs

### Slice 6 (complete)
Freeze whether Milestone 2 includes only required routines or also required
data members.

Completion criteria:

- the required member surface is explicit

### Slice 7 (complete)
Freeze the exact syntax for type-side conformance claims.

Completion criteria:

- one canonical conformance declaration shape is documented

### Slice 8 (complete)
Freeze unsupported standards surfaces that must still fail in Milestone 2.

Target behavior:

- unsupported standard kinds still fail
- generic-constrained standards still fail
- extension/disptach-only surfaces still fail

Completion criteria:

- negative tests pin those rejections

## Epoch 3: Resolver Representation For Standards

### Slice 9 (complete)
Add resolver-owned representation for top-level standard declarations.

Completion criteria:

- resolved program metadata can carry standards as named symbols

### Slice 10 (complete)
Thread standard identities into the relevant resolved package/program
structures.

Completion criteria:

- later phases can inspect standards without re-parsing syntax

### Slice 11 (complete)
Bind required standard members into standard-local semantic scope.

Completion criteria:

- required member declarations resolve within their standard body

### Slice 12 (complete)
Reject duplicate standard names and duplicate required member names cleanly.

Completion criteria:

- exact diagnostics exist for repeated standard/member declarations

### Slice 13 (complete)
Reject conformance references to unknown or out-of-scope standards explicitly.

Completion criteria:

- bad contract references fail locally and clearly

## Epoch 4: Typecheck Representation

### Slice 14 (complete)
Introduce typecheck-owned representation for standards.

Completion criteria:

- typed metadata can represent the chosen contract subset directly

### Slice 15 (complete)
Preserve required routine signatures in typed standard metadata.

Completion criteria:

- required routines survive into typecheck-owned structures

### Slice 16 (complete)
If included, preserve required data members in typed standard metadata.

Completion criteria:

- required data survives into typed contract structures

### Slice 17 (complete)
Represent type-side conformance declarations in typed type metadata.

Completion criteria:

- typed type declarations can carry claimed standards explicitly

### Slice 18 (complete)
Reject unsupported standard-member shapes with exact diagnostics.

Completion criteria:

- typecheck does not silently degrade unsupported standard bodies

## Epoch 5: Conformance Checking Core

### Slice 19 (complete)
Implement required-routine conformance checking for the simplest valid cases.

Completion criteria:

- positive tests prove one type can satisfy one standard through receiver
  routines

### Slice 20 (complete)
Implement missing-required-routine diagnostics.

Completion criteria:

- missing routine requirements fail explicitly and locally

### Slice 21 (complete)
Implement routine-signature mismatch diagnostics for conformance.

Completion criteria:

- wrong parameter or return shapes fail explicitly

### Slice 22 (complete)
If included, implement required-data conformance checking.

Completion criteria:

- positive and negative tests cover required data fulfillment

### Slice 23 (complete)
Reject under-specified or malformed conformance claims.

Completion criteria:

- contract claims fail clearly when the milestone does not support the shape

## Epoch 6: Boundaries With Existing Routine And Type Semantics

### Slice 24 (complete)
Pin that standards do not create object-method semantics in Milestone 2.

Completion criteria:

- tests/docs show conformance remains procedural

### Slice 25 (complete)
Reject using a standard as an ordinary concrete type unless the milestone
explicitly chooses such support.

Completion criteria:

- standards-as-values/types either work deliberately or fail explicitly

### Slice 26 (complete)
Reject generic-constrained standard use that belongs to later `V2`.

Completion criteria:

- generic + standards interactions outside the chosen subset fail honestly

### Slice 27 (complete)
Reject extension/disptach surfaces that are still later-milestone work.

Completion criteria:

- unsupported contract-based dispatch paths fail explicitly

### Slice 28 (complete)
Pin ambiguity behavior where multiple receiver-qualified routines might appear
to satisfy a contract.

Completion criteria:

- ambiguity failures are distinct from missing-member failures

## Epoch 7: Negative Boundary Hardening

### Slice 29 (complete)
Add compile-fail fixtures for unsupported standard kinds that remain outside the
chosen subset.

Completion criteria:

- unsupported `blu`/`ext` or other deferred forms still fail explicitly

### Slice 30 (complete)
Add compile-fail fixtures for malformed standard bodies.

Completion criteria:

- duplicate members, bad member kinds, and malformed signatures fail clearly

### Slice 31 (complete)
Add compile-fail fixtures for malformed conformance declarations.

Completion criteria:

- bad type-side contract claims fail explicitly

### Slice 32 (complete)
Add compile-fail fixtures for editor-facing standard misuse diagnostics.

Completion criteria:

- negative editor paths are pinned for the chosen standards subset

## Epoch 8: Lowering And Backend Honesty

### Slice 33 (complete)
Audit whether the chosen standards subset requires lowering support or should
stop before lower/backend with an explicit diagnostic.

Completion criteria:

- there is no fake “typecheck yes, lower maybe” path

### Slice 34 (complete)
If lowering support is needed for the chosen subset, implement the narrow path.

Completion criteria:

- supported standard examples survive lowering cleanly

### Slice 35 (complete)
If backend support is not yet viable, fail before backend with exact messaging.

Completion criteria:

- unsupported later-stage contract behavior fails honestly

## Epoch 9: Editor And Tree-Sitter Audit

### Slice 36 (complete)
Audit tree-sitter grammar/highlighting for the chosen standards syntax.

Completion criteria:

- standard/conformance syntax is either highlighted correctly or explicitly
  unchanged and verified

### Slice 37 (complete)
Audit `fol-editor` completion/hover/diagnostics for the chosen contract subset.

Completion criteria:

- editor coverage exists for the implemented standard/conformance subset

### Slice 38 (complete)
Add editor regression coverage for negative contract misuse.

Completion criteria:

- editor diagnostics stay aligned with compiler-backed truth

## Epoch 10: Examples, Book, And Closure

### Slice 39 (complete)
Add one canonical positive example package for the chosen standard/conformance
subset.

Completion criteria:

- one real example exists and is exercised in integration tests

### Slice 40 (complete)
Add one canonical negative example package for unsupported contract surfaces.

Completion criteria:

- one real negative example exists and is exercised in integration tests

### Slice 41
Update the standards chapter so it distinguishes:

- implemented Milestone 2 subset
- still-future `V2` contract design

Completion criteria:

- the book is honest about what now works and what still does not

### Slice 42
Update version-boundary docs and contributor guidance for the new Milestone 2
state.

Completion criteria:

- repo docs no longer describe the implemented subset as entirely future

### Slice 43
Run targeted contract/compiler/editor suites and confirm green Milestone 2
state.

Completion criteria:

- targeted standards/conformance tests pass

### Slice 44
Run repo gate.

Completion criteria:

- `make build` passes
- `make test` passes

### Slice 45
Commit Milestone 2 and mark this plan complete.

Completion criteria:

- committed with one conventional-commit title only
- `PLAN.md` fully marked complete
- worktree left clean except unrelated user-owned changes
