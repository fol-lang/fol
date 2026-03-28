# V2 Top-Level Roadmap

This file is the high-level roadmap for `V2`.

It is intentionally not a detailed slice plan.
It is the grouping document that says which major feature blocks `V2` should be
split into before detailed implementation plans are written.

The current `V2` direction should stay aligned with:

- [plan/VERSIONS.md](/home/bresilla/data/code/bresilla/fol/plan/VERSIONS.md)
- [book/src/500_items/400_standards.md](/home/bresilla/data/code/bresilla/fol/book/src/500_items/400_standards.md)
- [book/src/500_items/500_generics.md](/home/bresilla/data/code/bresilla/fol/book/src/500_items/500_generics.md)

The core rule stays the same:

- parser support is not enough
- a `V2` feature counts only when it works through the real compiler chain that
  matters for it

That means:

- syntax
- resolver behavior
- typecheck behavior
- diagnostics
- later stages needed by the feature
- editor/tree-sitter audit
- book and example honesty

## Group 1: Generics Core

This is the first `V2` block.

It should create the smallest useful generic system that can be enforced by the
compiler end to end without pretending the full later book surface already
exists.

Scope:

- generic routine declarations
- generic parameter binding
- generic references in routine signatures
- generic call resolution in a narrow useful form
- explicit diagnostics for unsupported generic uses

Goal:

- one real generic routine model
- no parser-only fake support
- no accidental promotion of full later generic design before semantics exist

This group should be implemented first because the later `V2` abstraction
systems depend on it.

## Group 2: Standards And Conformance

This is the second `V2` block.

It should make standards real as compiler-backed contracts rather than book-only
syntax sketches.

Scope:

- `std ...: pro`
- `std ...: blu`
- `std ...: ext`
- required routine checking
- required data checking
- type-to-standard satisfaction checks
- explicit conformance diagnostics

Goal:

- standards become real named contracts
- conformance is enforced, not implied
- receiver-qualified routine fulfillment works coherently

This group should follow generics core because standards and conformance need
stronger type-level reasoning and later interact with constrained dispatch.

## Group 3: Dispatch, Inference, And Call Semantics

This is the third `V2` block.

It should make generic and standard-based code usable instead of merely legal.

Scope:

- constrained generic call resolution
- generic argument inference
- standard-aware dispatch
- ambiguity detection
- error reporting for conflicting or underconstrained calls
- interaction with current routine calls and receiver sugar

Goal:

- generic and standard-based calls resolve predictably
- ambiguous cases fail with exact diagnostics
- the language remains procedural, not object-dispatch based

This group should not start until Group 1 and Group 2 have a real semantic
foundation.

## Group 4: Tooling, Book, And End-to-End Closure

This is the last `V2` block, but it is not optional cleanup.

It is the closure block that turns the chosen `V2` subset into a real project
contract instead of an internal compiler experiment.

Scope:

- lowering/backend coverage for the chosen `V2` subset
- `fol-editor` audit
- tree-sitter grammar/query updates
- examples and negative fixtures
- docs and book updates
- version-boundary updates from “future design” to “implemented subset”

Goal:

- the chosen `V2` subset is honest across the whole repo
- editor/tree-sitter does not lag behind syntax or semantics
- the book says exactly what is implemented and what still belongs to later `V2`

## Recommended order

1. Generics Core
2. Standards And Conformance
3. Dispatch, Inference, And Call Semantics
4. Tooling, Book, And End-to-End Closure

## Immediate next step

The next detailed plan should be only for:

- Group 1: Generics Core

That should stay narrow and compiler-honest, and it should not silently absorb
the later standards/conformance/dispatch work.
