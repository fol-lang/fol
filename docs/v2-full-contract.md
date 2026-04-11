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
