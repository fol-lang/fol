# Generics

This chapter describes `V2` generic-language work rather than current `V1`
compiler behavior.

Current milestone note:

- Milestone 1 now includes a narrow generic-routine subset through:
  - parser
  - resolver
  - typecheck
  - editor audit
- that current subset supports:
  - generic routine declarations
  - type parameters
  - generic parameter references in parameter and return types
  - direct calls with narrow argument-driven inference
- current hardened example set for that subset is:
  - `examples/generic_routine_m1`
  - `examples/generic_routine_pair_m1`
  - `examples/generic_routine_cross_file_m1`
  - `examples/generic_receiver_m1`
  - `examples/generic_receiver_cross_file_m1`
  - `examples/generic_receiver_overload_m1m2`
  - `examples/generic_type_semantic_m1m2`
    - positive semantic-check fixture for generic type declarations
  - `examples/generic_type_exec_m1m2`
  - `examples/generic_standard_constraint_m1m2`
  - `examples/generic_standard_constraint_generic_m1m2`
  - `examples/fail_generic_misuse_m1`
  - `examples/fail_generic_cross_file_m1`
  - `examples/fail_generic_receiver_m1`
  - `examples/fail_generic_standard_constraint_m1m2`
- generic routine lowering now succeeds for the shipped Milestone 1 example set
- generic routine backend execution now works for the shipped positive Milestone 1 examples
- ownership at a generic call boundary follows the concrete argument: clone-safe
  arguments remain available to the caller, while owned values and unique
  pointers move. Inside a routine, an unconstrained generic parameter is
  conservatively move-only because the routine cannot prove that every future
  instantiation is clone-safe; forwarding it therefore moves rather than
  deep-cloning a unique value
- an unresolved generic parameter cannot cross `[>]` or `| async` until FOL
  defines a thread-safety and lifetime contract for generics; calls whose
  arguments infer concrete thread-safe types remain valid
- receiver-qualified generic routines, matching default arguments, and
  concrete instantiated generic-type receivers and concrete recoverable error
  types are now part of the executable Milestone 1 subset
- generic receiver routines are now current contract: a routine such as
  `fun (Box[T])get(T)(): T` declares its generic parameters explicitly,
  binds them through the receiver and ordinary arguments at each call site,
  and monomorphizes into one concrete routine per instantiation
- receiver-qualified routine bodies read receiver state through `self`,
  which is typed as the receiver and lowered as the routine's first argument
- generic type execution and constrained generic execution now have checked-in
  example packages too
- nested generic-type composition now works for the checked nested-record
  subset such as `Box[Box[int]]`
- generic type instantiations carry nominal identity: `Box[int]` and `Cup[int]`
  are distinct types even with an identical field shape, so a method shared by
  name dispatches to each base's own receiver routine rather than being
  rejected as ambiguous; `examples/generic_receiver_overload_m1m2` pins this
- broader Milestone 1 edge-case policy is still tracked separately from that
  shipped positive core
- current editor hardening covers hover/definition on checked-in generic
  examples without claiming broader generic-aware completion than the shipped
  editor currently provides
- generic error shells remain outside the current shipped Milestone 1 routine subset
- recursive value edges remain rejected because they have no finite inline
  layout. V3 owned heap recursion is now supported: an edge such as
  `opt @Node` lowers nominally to `Option<Box<Node>>`. Recursive generic
  instantiation through an inline container remains outside the shipped generic
  subset, pinned by `examples/fail_generic_recursive_m1m2`
- generic types are not part of `V1`; they now belong to the shipped narrow
  full-`V2` contract instead
- examples here should be read as:
  - current narrow `V2` Milestone 1 generic-routine work where noted
  - otherwise later `V2` design

## Types

### Generic Functions

Generic programming aims to express one routine over a family of concrete
types, with the requirements written explicitly in the signature.


```
pro max[T: gen](a, b: T): T = {
	result =  a | a < b | b;
};
fun biggerFloat(a, b: flt[32]): flt[32] = { max(a, b) }
fun biggerInteger(a, b: int[64]): int[64] = { max(a, b) }
```

### Generic Types

Full `V2` now includes generic type declarations and explicit instantiation.

Chosen contract:

- the canonical declaration surface follows the existing parser-owned shape
- generic records and generic aliases are in scope for full `V2`
- generic arguments are explicit and arity-checked
- generic types use the same monomorphization-oriented execution strategy as
  generic routines

Example declaration shapes:

```fol
typ Box(T: item): rec = {
    value: T;
};

typ Pair(T: left, U: right): map[T, U];
```

## Generic Calls

Full `V2` includes standards-as-constraints, but not broad dispatch semantics.

Chosen constraint contract:

- protocol standards are the only generic-constraint surface in the full `V2`
  target
- a constraint may name a generic standard at concrete arguments
  (`fun drive(T: Holder[int])(...)`): the standard's own generic parameters are
  bound to those arguments, so a constraint call substitutes them (a required
  `fun fetch(): Item` on `Holder[int]` types as `int`), and the conformer must
  claim the standard at the same arguments; `examples/generic_standard_constraint_generic_m1m2`
  pins this
- constrained generic calls remain procedural and are checked through declared
  conformance
- standards-as-constraints do not imply runtime object dispatch

This chapter still does not define an object-dispatch system. If later work
uses receiver-qualified routine syntax together with constraints, that remains a
procedural call-binding rule unless the contract changes explicitly.

```fol
std foo: pro = { fun bar(); }

typ[ext] int, str: int, str;

fun (int)bar() = {  }
fun (str)bar() = {  }

pro callBar(T: foo)(value: T) = { value.bar() }             // dispatch with generics
pro barCall( value: foo ) = { value.bar() }                 // dispatch with standards

pro main: int = {
    callBar(2);
    callBar("go");

    barCall(2);
    barCall("go")
}
```
