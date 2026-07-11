# V2 Full Contract

This note freezes what "full V2" means for the current FOL codebase.

It does not mean every later example in the book.
It does not include the broader dispatch/inference direction shown in some
future-facing examples.

Current full `V2` target:

- executable generic routines
- generic types
- executable protocol standards
- standards-as-constraints

Still out of scope for this `V2` target:

- blueprint standards
- extended standards
- standards as ordinary concrete value types
- broad dispatch driven by standards
- broader inference driven by standards
- object-style dispatch semantics

The implementation rule is:

- land one exact compiler/runtime/editor contract for the surfaces above
- keep later dispatch-oriented work outside `V2` until chosen explicitly

## Generic Type Contract

Full `V2` includes generic types.

Chosen contract:

- generic type declarations are part of full `V2`
- the canonical declaration surface follows the existing parser-owned shape, for
  example `typ Box(T: item): rec = { ... };`
- generic records and generic aliases are part of the target
- generic argument arity must be explicit and exact
- generic types remain a compile-time instantiation surface under the chosen
  monomorphization strategy

Still outside this generic-type contract:

- generic inference for type arguments by unrelated contextual usage
- generic constraints beyond standards-as-constraints
- a second runtime-owned reified generic type system

## Constraint Contract

Full `V2` includes standards-as-constraints.

Chosen contract:

- generic constraints are expressed through standards
- protocol standards are the only constraint surface in the full `V2` target
- constrained generic routines remain procedural call binding, not object
  dispatch
- constraint satisfaction is checked statically through declared conformance

Still outside this constraint contract:

- non-standard generic constraint languages
- blueprint standards as constraints
- extended standards as constraints
- dispatch or inference driven by constraints

## Blueprint Standards

Blueprint standards are part of the shipped full `V2` contract
(workstream M).

Decision:

- `std X: blu` declares required data fields as a static contract
- conformance is checked at type declaration time against the conformer's
  record fields
- there is no data inheritance and no runtime component; blueprints stay
  purely compile-time field-shape checks
- `examples/standards_blueprint_m2` is the canonical positive example and
  `examples/fail_standard_blueprint_m2` pins the rejection wording

## Extended Standards

Extended standards are part of the shipped full `V2` contract
(workstream N).

Decision:

- `std X: ext` combines required routines and required fields in one
  standard
- conformance checks both halves with the same static machinery as
  protocol and blueprint standards
- `examples/standards_extended_m2` is the canonical positive example

## Dispatch And Inference

Broader dispatch and inference semantics are not part of the full `V2` target.

Decision:

- full `V2` does not include standards-driven dispatch
- full `V2` does not include broader inference driven by standards
- constrained generic calls remain procedural and statically checked
- future dispatch-oriented examples in the book should be read as later work,
  not current `V2`
