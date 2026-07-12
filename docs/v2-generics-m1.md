# V2 Generics Milestone 1

Historical transition note:

- this document tracks the narrow M1 hardening state, not the full `V2`
  product contract
- the current full `V2` target is frozen in `docs/v2-full-contract.md`

This note freezes the intended scope for the first `V2` generics milestone.

It is not the full `V2` generic design.
It is not the standards/conformance plan.
It is not the later dispatch and inference plan.

The goal of Milestone 1 is narrower:

- make one honest generic-routine core real end to end

## Included target

Milestone 1 is about generic routines only.

The intended supported subset is:

- generic routine declarations
- type parameters only
- generic parameter references in routine parameter types
- generic parameter references in routine return types
- direct routine calls with narrow argument-driven inference only

Milestone 1 call contract:

- direct calls like `pick(1)` are the only supported generic call path
- explicit generic-call syntax is not part of Milestone 1
- contextual return-type inference is not part of Milestone 1
- generic routines are not first-class plain routine values in Milestone 1
- postfix template-call syntax such as `value$` is a separate parser surface and
  remains unsupported for generic-call semantics

The compiler should only claim this milestone when the feature is real across:

- parser
- resolver
- typecheck
- diagnostics
- later stages needed by the chosen subset
- editor/tree-sitter audit

## Explicitly out of scope

The following stay outside Milestone 1:

- generic type declarations
- standards/protocol conformance
- blueprints
- extensions
- rich generic constraint solving
- advanced generic inference
- broad dispatch work
- object-style dispatch interpretation

## Current implemented Milestone 1 truth

At the current repo state after the landed Milestone 1 semantic slices:

- parser accepts generic routine declarations in the chosen narrow shape
- parser owns duplicate generic-parameter name rejection before resolver
- resolver binds routine-local generic parameter symbols in supported type positions
- typecheck supports direct generic routine calls with narrow argument-driven inference
- generic routine values remain unsupported
- generic routine lowering now succeeds for the shipped Milestone 1 examples
- backend execution now works for the shipped positive Milestone 1 examples
- receiver-qualified generic routines now lower and execute through method sugar
- concrete instantiated generic-type receivers such as `Box[int]` now lower and execute too
- generic receiver routines such as `fun (Box[T])get(T)(): T` now lower,
  monomorphize into concrete per-instantiation clones, and execute
- receiver-qualified routine bodies can read receiver state through `self`
- generic type templates are recorded for every source unit before other
  declarations lower, so cross-file generic instantiation no longer depends
  on file ordering
- default-argument generic routines are part of the current executable M1 subset
- generic routines with concrete recoverable error types are part of the current executable M1 subset
- full `V2` execution examples now also exist for:
  - `examples/generic_type_exec_m1m2`
  - `examples/generic_standard_constraint_m1m2`
- generic type declarations now use the positively named semantic-check fixture
  `examples/generic_type_semantic_m1m2`
- nested generic-type composition now typechecks and executes for the checked
  nested-record subset such as `Box[Box[int]]`
- generic type instantiations now carry nominal identity: `Box[int]` and
  `Cup[int]` stay distinct types even when they share a field shape, so a
  method shared by name dispatches to each base's own receiver routine instead
  of gating as ambiguous; runtime-pinned by
  `examples/generic_receiver_overload_m1m2`
- a generic standard now works as a generic-parameter constraint
  (`fun drive(T: Holder[int])(...)`): the standard's own parameters are bound to
  the constraint arguments, so a constraint call substitutes them (a required
  `fun fetch(): Item` on `Holder[int]` types as `int`) and the conformer must
  claim the standard at matching arguments; runtime-pinned by
  `examples/generic_standard_constraint_generic_m1m2`

That means the current honest boundary is:

- parser/resolver/typecheck fixtures, lowered snapshots, editor-opened
  examples, and compile-and-run examples are the current validation path for
  Milestone 1 generic routine examples
- the chosen executable M1 edge cases are now:
  - receiver-qualified generic routines
  - concrete instantiated generic-type receivers
  - default-argument generic routines
  - generic routines with concrete recoverable error types
- generic error shells remain explicitly unsupported in M1
- no narrowing slice should pretend resolver owns duplicate generic-name
  diagnostics when parser already rejects them first
- no narrowing slice should claim all generic edge cases work in Milestone 1
  before those cases are chosen and tested explicitly

## Immediate implementation rule

Milestone 1 should not silently absorb later `V2` work.

In particular:

- no generic types
- no standards-as-constraints semantics inside the narrow Milestone 1 core
- no full inference
- no explicit generic-call syntax
- no broad dispatch rules

If those surfaces are parsed but not part of the chosen semantic subset, they
must fail explicitly and locally.

## Hardening obligations

Milestone 1 is already implemented narrowly enough that hardening now matters
more than widening.

Positive obligations:

- parser must keep accepted generic routine headers stable
- resolver must keep generic parameters routine-local and visible in supported
  type positions
- typecheck must keep direct argument-driven inference stable for the current
  subset
- editor-opened example packages must remain clean through parse/resolve/typecheck

Negative obligations:

- malformed generic headers must fail clearly in parser
- duplicate generic parameter names remain parser-owned
- generic parameter references outside routine scope must fail in resolver
- generic routine values must fail in typecheck
- non-standard generic constraints must fail explicitly
- lowering must continue to succeed for the shipped examples
- backend execution must continue to succeed for the shipped positive examples
- broader edge cases must keep explicit tests and explicit acceptance/rejection

Hardening examples that should remain in sync:

- positive
  - `examples/generic_routine_m1`
  - `examples/generic_routine_pair_m1`
  - `examples/generic_routine_cross_file_m1`
  - `examples/generic_receiver_m1`
  - `examples/generic_receiver_cross_file_m1`
  - `examples/generic_type_semantic_m1m2`
  - `examples/generic_type_exec_m1m2`
  - `examples/generic_standard_constraint_m1m2`
- negative
  - `examples/fail_generic_misuse_m1`
  - `examples/fail_generic_cross_file_m1`
  - `examples/fail_generic_receiver_m1`
  - `examples/fail_generic_standard_constraint_m1m2`

Current hardened example matrix:

- positive lowered examples
- positive executable examples beyond the narrow M1 core
  - `examples/generic_type_exec_m1m2`
  - `examples/generic_standard_constraint_m1m2`
- positive executable generic-receiver examples
  - `examples/generic_receiver_m1`
  - `examples/generic_receiver_cross_file_m1`
- positive semantic-check or lowered examples
  - `examples/generic_routine_m1`
  - `examples/generic_routine_pair_m1`
  - `examples/generic_routine_cross_file_m1`
  - `examples/generic_type_semantic_m1m2`
- negative semantic-boundary examples
  - `examples/fail_generic_misuse_m1`
  - `examples/fail_generic_cross_file_m1`
  - `examples/fail_generic_receiver_m1`
  - `examples/fail_generic_standard_constraint_m1m2`

## Second-pass hardening targets

The first hardening pass covered the broad Milestone 1 contract.
The second pass is narrower and deeper.

Current hardening targets:

- receiver-qualified generic routines
  - parser, typecheck, lowering, and runtime now pin the current truth
- richer signature-position generic usage
  - nested optional, error-shell, and container positions must keep explicit
    current behavior
- default arguments in generic routines
  - executable behavior is now pinned for matching inference cases
- concrete recoverable error types in generic routines
  - executable behavior is now pinned for current `check(...)` usage
- imported and cross-file generic routine calls
  - `loc`/workspace cases must be pinned independently from single-file cases
- editor and tree-sitter coverage depth
  - checked-in generic examples need deeper real-example coverage, not only
    open-cleanly checks

Current deeper hardening boundaries now pinned too:

- generic hover and definition on checked-in examples are covered
- plain completion still does not pretend generic-smart suggestions where the
  current editor does not provide them
- nested generic-type composition is pinned for the checked nested-record
  subset without widening into recursive generic self-instantiation
- the negative generic-constraint conformance example is:
  - `examples/fail_generic_standard_constraint_m1m2`

## Current documented narrow limits

The shipped narrow `V2` surface still keeps these limits explicit:

- underconstrained generic calls are rejected instead of using broader
  solver-style inference
- generic routines are not first-class routine values
- generic error shells remain unsupported in the Milestone 1 routine subset
- recursive generic type instantiation remains rejected — and so does any
  recursive type definition (generic or plain), whether the self-reference is
  direct or through a container/another type; the checker now reports an honest
  `recursive type '...' is not yet supported` boundary instead of overflowing in
  lowering, pinned by `examples/fail_generic_recursive_m1m2`

If any of these limits change, they should move through a dedicated
implementation slice with parser, typecheck, editor, doc, and example updates
rather than disappearing from the contract silently.

Second-pass hardening must not widen Milestone 1 into:

- generic constraints
- generic types
- first-class generic routine values
- broad generic edge-case support without explicit tests
