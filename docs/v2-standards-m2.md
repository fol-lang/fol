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

## Current pre-implementation truth

At the current repo state before Milestone 2 semantics land:

- parser already accepts protocol, blueprint, and extended standards
- parser already accepts explicit type-side contract headers
- resolver already collects top-level standard symbols
- resolver already resolves type-side contract references against those symbols
- typecheck still rejects standards and type contract conformance as future work

That means the immediate job is honesty and controlled narrowing:

- document the supported target subset
- freeze the current parser/resolver truth with tests
- then implement the semantic subset deliberately

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
