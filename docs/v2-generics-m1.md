# V2 Generics Milestone 1

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
- generic types remain unsupported
- generic routine lowering is still explicitly unsupported

That means the current honest boundary is:

- parser/resolver/typecheck fixtures and editor-opened examples are the current
  validation path for Milestone 1 generic routine examples
- full lowering/backend execution still stops with an explicit generic-routine diagnostic
- no narrowing slice should pretend resolver owns duplicate generic-name
  diagnostics when parser already rejects them first
- no narrowing slice should claim generic lowering works in Milestone 1
  while the chosen boundary is still an explicit lowering stop

## Immediate implementation rule

Milestone 1 should not silently absorb later `V2` work.

In particular:

- no generic types
- no standards-as-constraints semantics
- no full inference
- no explicit generic-call syntax
- no broad dispatch rules

If those surfaces are parsed but not part of the chosen semantic subset, they
must fail explicitly and locally.
