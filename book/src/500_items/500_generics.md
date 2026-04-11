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
  - `examples/fail_generic_type_m1`
  - `examples/fail_generic_misuse_m1`
  - `examples/fail_generic_cross_file_m1`
  - `examples/fail_generic_standard_constraint_m1m2`
- generic routine lowering/backend support is still explicitly out of scope for
  the current Milestone 1 state
- current editor hardening covers hover/definition on checked-in generic
  examples without claiming broader generic-aware completion than the shipped
  editor currently provides
- generic types are not part of the implemented `V1` typechecker
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

This chapter does not define an object-dispatch system. If generics later use
receiver-qualified routine syntax, that would still be procedural call binding,
not virtual methods or inheritance.

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
