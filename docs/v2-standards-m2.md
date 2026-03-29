# V2 Standards Milestone 2

This note freezes the intended scope for the second `V2` milestone.

It follows the completed generic-routine Milestone 1.
It is not the whole remaining `V2` contract design.
It is not the later dispatch and inference plan.

The goal of Milestone 2 is narrower:

- make one honest standards/conformance core real end to end

## Included target

Milestone 2 starts with protocol standards first.

The intended supported subset is:

- named protocol standards declared with `std name: pro = { ... }`
- required receiver-qualified routine signatures only
- type-side conformance headers written through the existing type-contract
  syntax, for example `typ Rect()(geo): rec = { ... }`

Milestone 2 contract note:

- `pro` is the first supported standard kind
- required routines are the first supported contract members
- required data members remain out of scope for the first semantic batch
- conformance remains procedural, not object-dispatch based
- standards are not ordinary concrete value or parameter types
- exact routine-signature matching is required for conformance

## Explicitly out of scope

The following stay outside Milestone 2 unless a later slice changes the scope
deliberately:

- blueprint standards as semantic contracts
- extended standards as semantic contracts
- standards with generic constraints
- standards as concrete ordinary value types
- dispatch and inference based on standards
- object-style method dispatch
- richer extension machinery

## Current implemented truth

At the current repo state after the landed Milestone 2 semantic slices:

- parser accepts protocol, blueprint, and extended standards
- parser accepts explicit type-side contract headers
- resolver collects top-level standard symbols
- resolver resolves type-side contract references against those symbols
- typecheck implements protocol-only procedural conformance for required
  receiver-qualified routines
- blueprint and extended standards remain explicitly unsupported
- lowering/backend still stop at an explicit Milestone 2 boundary

## Immediate implementation rule

Milestone 2 should not silently absorb later `V2` work.

In particular:

- no blueprint semantics until chosen explicitly
- no extended-standard semantics until chosen explicitly
- no standards-as-dispatch
- no generic-constrained standards
- no pretending parser support alone means contracts are implemented

If those surfaces are parsed but not part of the chosen semantic subset, they
must fail explicitly and locally.

## Hardening obligations

Milestone 2 is now at the stage where edge-case coverage matters more than
widening.

Positive obligations:

- parser must keep accepted protocol-standard syntax stable
- resolver must keep standard symbols and type-side claims stable across files
- typecheck must keep protocol conformance stable for the current subset
- editor-opened example packages must remain clean through parse/resolve/typecheck

Negative obligations:

- malformed standard bodies must fail clearly
- malformed conformance claims must fail clearly
- unsupported `blu` and `ext` claims must fail clearly
- standards used as ordinary types must fail clearly
- generic constraints using standards must fail clearly
- lowering/backend must continue to stop with one exact Milestone 2 boundary

Hardening examples that should remain in sync:

- positive
  - `examples/standards_protocol_m2`
  - `examples/standards_protocol_pair_m2`
- negative
  - `examples/fail_standard_blueprint_m2`
  - `examples/fail_standard_as_type_m2`

Current hardened example matrix:

- positive lowering-boundary examples
  - `examples/standards_protocol_m2`
  - `examples/standards_protocol_pair_m2`
- negative semantic-boundary examples
  - `examples/fail_standard_blueprint_m2`
  - `examples/fail_standard_as_type_m2`

## Second-pass hardening targets

The first hardening pass covered the broad Milestone 2 protocol subset.
The second pass is focused on deeper edge-case truth.

Current hardening targets:

- multi-standard conformance on one type
  - resolver, typecheck, and examples must all agree on the exact subset
- imported-standard conformance truth
  - cross-file and imported claims must be pinned at typecheck, not only
    resolver
- unsupported requirement-shape diagnostics
  - each unsupported required-routine shape should keep one exact message path
- deeper editor and tree-sitter coverage
  - standards examples need more than open-cleanly and highlight-only checks

Second-pass hardening must keep these surfaces out of scope:

- blueprint standards
- extended standards
- standards as ordinary concrete types
- lowering/backend support beyond the explicit M2 boundary
