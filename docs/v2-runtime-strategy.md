# V2 Runtime And Backend Strategy

This note freezes the execution strategy for the current full `V2` target.

Chosen strategy:

- monomorphization for executable generic routines
- monomorphization for generic type instantiations
- monomorphized conformance-aware code generation for standards-as-constraints

This means:

- lowering should resolve supported generic and constrained uses into concrete
  instantiated lowered routines and lowered types
- backend emission should not introduce a second witness/dictionary calling
  convention for the current `V2` target
- standard-constrained calls should become ordinary emitted receiver-qualified
  or monomorphized routine calls after compile-time conformance checking
- protocol standards should remain procedural contracts, not a runtime object
  model
- `fol-runtime` remains a runtime support crate, not a second generic or
  standards object system

Explicit non-goals for this strategy:

- no dictionary passing for the current `V2` target
- no hybrid generic dispatch model
- no object-style runtime vtables

Implementation rule:

- `fol-lower` and `fol-backend` should use one monomorphization model
- tests and snapshots should describe instantiated lowered routines/types rather
  than mixed strategy output
