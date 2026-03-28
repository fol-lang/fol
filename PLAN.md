# V2 Milestone 1 Plan

This file is the detailed execution plan for:

- `V2`
- Group 1
- Generics Core

It is intentionally narrow.

It is not the whole of `V2`.
It does not include standards.
It does not include blueprints or extensions.
It does not include advanced dispatch beyond what is strictly needed to make
generic routines real and compiler-backed.

The milestone goal is:

- make one honest generic-routine core work end to end

That means:

- syntax is accepted deliberately
- resolver and typecheck understand it
- unsupported shapes fail explicitly
- editor/tree-sitter are audited
- docs/examples stop pretending broader generic support exists

The target subset for Milestone 1 is:

- generic routine declarations
- generic parameters in routine signatures
- generic parameter references in parameter and return types
- explicit generic call sites only if needed by the chosen syntax
- typecheck support for a narrow generic routine model

Out of scope for this milestone:

- generic type declarations
- standards/protocol conformance
- blueprint/ext surfaces
- rich constraint solving
- advanced inference
- any object-style dispatch interpretation

Success criteria:

- generic routines are either supported end to end or rejected explicitly
- no parser-only generic surfaces are left ambiguously “half working”
- `make build` passes
- `make test` passes

## Epoch 1: Freeze The Milestone Contract

### Slice 1 (complete)
Audit current V2 book/version wording for generics and restate the Milestone 1
subset in repo-local docs/tests.

Completion criteria:

- one exact repo-local statement exists for what Milestone 1 includes
- it explicitly excludes generic types and standards work

### Slice 2 (complete)
Add regression tests that freeze the current pre-implementation behavior for
generic surfaces.

Completion criteria:

- the suite proves current generic routine/type surfaces are not silently
  supported yet
- tests make future deltas visible

### Slice 3 (complete)
Audit parser coverage for generic routine syntax already accepted today.

Completion criteria:

- there is an explicit parser truth inventory for:
  - generic routine declarations
  - generic parameter lists
  - generic parameter references in type positions

### Slice 4 (complete)
Audit resolver/typecheck/lower/editor assumptions that currently reject or
ignore generic surfaces.

Completion criteria:

- known baseline blockers are pinned before semantic work starts

## Epoch 2: Choose The Exact Generic Core Shape

### Slice 5 (complete)
Freeze the supported generic declaration shape for Milestone 1.

Target behavior:

- generic routines have one exact supported header shape
- unsupported alternate shapes fail explicitly

Completion criteria:

- one canonical generic routine form is documented in tests/docs

### Slice 6 (complete)
Freeze the supported generic parameter kinds for Milestone 1.

Target behavior:

- type parameters only, unless a second kind is explicitly chosen
- no fake support for richer generic kinds yet

Completion criteria:

- supported generic parameter kinds are explicit

### Slice 7 (complete)
Freeze the supported generic type-reference positions for Milestone 1.

Target behavior:

- parameters
- returns
- maybe error types if chosen
- no extra positions unless implemented deliberately

Completion criteria:

- allowed positions are explicit in tests/docs

### Slice 8 (complete)
Freeze the unsupported generic surfaces that must still fail in Milestone 1.

Target behavior:

- generic types still fail
- standards-related generic constraints still fail
- advanced generic dispatch still fails

Completion criteria:

- negative tests pin those rejections

## Epoch 3: Resolver Representation

### Slice 9 (complete)
Add or tighten resolver-owned representation for generic routine parameters.

Completion criteria:

- resolved routine metadata can carry generic parameter facts

### Slice 10 (complete)
Thread generic parameter identities into the relevant resolved routine/program
structures.

Completion criteria:

- generic parameters are available to later phases without ad hoc re-parsing

### Slice 11 (complete)
Bind generic parameters into routine-local semantic scope.

Target behavior:

- generic names resolve in supported type positions within the routine

Completion criteria:

- generic names resolve where the milestone says they should

### Slice 12
Reject duplicate generic parameter declarations cleanly.

Completion criteria:

- exact resolver diagnostics exist for duplicate generic parameter names

### Slice 13 (complete)
Reject generic parameter references outside their supported scope.

Completion criteria:

- out-of-scope generic name use fails explicitly

## Epoch 4: Signature Lowering And Type References

### Slice 14 (complete)
Teach declaration-signature lowering to preserve generic parameter references in
routine signatures.

Completion criteria:

- generic parameter references survive lowering from syntax/resolution into
  typecheck-facing signature data

### Slice 15 (complete)
Support generic parameter references in routine parameter types.

Completion criteria:

- parameter positions accept generic references in the chosen subset

### Slice 16 (complete)
Support generic parameter references in return types.

Completion criteria:

- return positions accept generic references in the chosen subset

### Slice 17 (complete)
If included in the subset, support generic parameter references in declared
error types.

Completion criteria:

- error-type support is either implemented and tested or explicitly rejected

### Slice 18 (complete)
Reject unsupported generic parameter use sites with exact diagnostics.

Completion criteria:

- type lowering/typecheck does not silently degrade unsupported positions

## Epoch 5: Typecheck Core Semantics

### Slice 19 (complete)
Introduce typecheck-owned representation for generic routine signatures.

Completion criteria:

- typed routine metadata can represent generic signatures directly

### Slice 20 (complete)
Implement generic parameter substitution shape for a narrow routine call path.

Completion criteria:

- typecheck can instantiate or compare a generic routine in one supported way

### Slice 21 (complete)
Typecheck direct generic routine use in the simplest valid cases.

Completion criteria:

- positive tests prove generic identity-style or same-type routine cases work

### Slice 22 (complete)
Reject mismatched concrete argument use for generic routines.

Completion criteria:

- generic call mismatches fail explicitly and locally

### Slice 23 (complete)
Reject underconstrained generic routines that the milestone does not support.

Completion criteria:

- typecheck does not pretend unsupported inference exists

### Slice 24 (complete)
Reject unsupported mixed generic/plain routine interactions cleanly.

Completion criteria:

- diagnostics distinguish unsupported generic semantics from ordinary type
  mismatches

## Epoch 6: Call Surface And Inference Boundary

### Slice 25 (complete)
Freeze whether Milestone 1 requires explicit generic call arguments or only
supports inference-free same-type cases.

Completion criteria:

- one exact call contract is chosen and documented in tests

### Slice 26 (complete)
If explicit generic call syntax is already parsed, connect the minimal chosen
form through resolver/typecheck.

Completion criteria:

- one explicit generic call path works end to end, or remains explicitly
  rejected

### Slice 27 (complete)
Pin no-inference or narrow-inference behavior with regression tests.

Completion criteria:

- supported inference behavior is explicit
- unsupported inference behavior fails clearly

### Slice 28 (complete)
Reject ambiguous generic calls with exact diagnostics.

Completion criteria:

- ambiguity failures are distinct from unresolved-name/type-mismatch failures

## Epoch 7: Negative Boundary Hardening

### Slice 29 (complete)
Add compile-fail fixtures for generic type declarations remaining out of scope.

Completion criteria:

- generic type surfaces still fail explicitly

### Slice 30 (complete)
Add compile-fail fixtures for standards/protocol-style generic constraints
remaining out of scope.

Completion criteria:

- standards-related generic surfaces still fail explicitly

### Slice 31 (complete)
Add compile-fail fixtures for generic routine shapes the milestone does not
accept.

Completion criteria:

- unsupported headers or parameter forms fail explicitly

### Slice 32 (complete)
Add compile-fail fixtures for editor-facing generic misuse diagnostics.

Completion criteria:

- negative editor paths are pinned for the chosen generic subset

## Epoch 8: Lowering And Backend Honesty

### Slice 33 (complete)
Audit whether generic routines in the chosen subset need lowering support or
must stop before lower/backend with an explicit diagnostic.

Completion criteria:

- there is no fake “typecheck yes, lower maybe” path

### Slice 34
If lowering support is needed for the chosen subset, implement the narrow path.

Completion criteria:

- supported generic routine examples survive lowering cleanly

### Slice 35 (complete)
If backend support is not yet viable, fail before backend with exact messaging.

Completion criteria:

- unsupported later-stage generic behavior fails honestly

## Epoch 9: Editor And Tree-Sitter Audit

### Slice 36 (complete)
Audit tree-sitter grammar/highlighting for the chosen generic syntax.

Completion criteria:

- generic syntax is either highlighted correctly or explicitly unchanged and
  verified

### Slice 37 (complete)
Audit `fol-editor` completion/hover/diagnostics for the chosen generic subset.

Completion criteria:

- editor coverage exists for the implemented generic routine subset

### Slice 38 (complete)
Add editor regression coverage for negative generic misuse.

Completion criteria:

- editor diagnostics stay aligned with compiler-backed truth

## Epoch 10: Examples, Book, And Closure

### Slice 39 (complete)
Add one canonical positive example package for generic routines in the chosen
Milestone 1 subset.

Completion criteria:

- one real example exists and is exercised in integration tests

### Slice 40 (complete)
Add one canonical negative example package for unsupported generic surfaces.

Completion criteria:

- one real negative example exists and is exercised in integration tests

### Slice 41 (complete)
Update the generics chapter so it distinguishes:

- implemented Milestone 1 subset
- still-future V2 generic design

Completion criteria:

- the book is honest about what now works and what still does not

### Slice 42 (complete)
Update version-boundary docs and contributor guidance for the new Milestone 1
state.

Completion criteria:

- repo docs no longer describe the implemented subset as entirely future

### Slice 43 (complete)
Run targeted generic/compiler/editor suites and confirm green Milestone 1 state.

Completion criteria:

- targeted tests for generics pass

### Slice 44
Run repo gate.

Completion criteria:

- `make build` passes
- `make test` passes

### Slice 45
Commit Milestone 1 and mark this plan complete.

Completion criteria:

- committed with one conventional-commit title only
- `PLAN.md` fully marked complete
- worktree left clean except unrelated user-owned changes
