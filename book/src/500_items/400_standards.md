# Standards

This chapter now has a split status:

- one narrow `V2` Milestone 2 subset is implemented
- the broader standards design is still future `V2`

Current implemented Milestone 2 subset:

- protocol standards declared with `std name: pro = { ... }`
- required receiver-qualified routine signatures only
- type-side conformance claims through the existing contract-header shape
- exact routine-signature matching for conformance
- current hardened example set for that subset is:
  - `examples/standards_protocol_m2`
  - `examples/standards_protocol_pair_m2`
  - `examples/standards_protocol_multi_m2`
  - `examples/fail_standard_blueprint_m2`
  - `examples/fail_standard_as_type_m2`
  - `examples/fail_standard_missing_routine_m2`
  - `examples/fail_standard_signature_m2`
  - `examples/fail_standard_import_ambiguity_m2`

Still future:

- blueprint standards as real semantic contracts
- extended standards as real semantic contracts
- required data-member conformance
- standards as ordinary concrete types
- generic constraints using standards
- dispatch/inference driven by standards
- object-style method semantics

The intent of standards is procedural and data-oriented. They are not class
hierarchies, inheritance trees, or object systems.

## Standard

In later milestones, a standard is intended to be a named collection of
required receiver-qualified routine signatures and/or required data, created
with `std`.

```fol
std geometry: pro = {
    fun area(): flt[64];
    fun perim(): flt[64];
};
```

The full planned forms are:

- protocol `pro[]` for required routines
- blueprint `blu[]` for required data
- extended `ext[]` for routines plus data

```fol
std geometry: pro = {
    fun area(): flt[64];
    fun perim(): flt[64];
};

std geometry: blu = {
    var color: rgb;
    var size: int;
};

std geometry: ext = {
    fun area(): flt[64];
    fun perim(): flt[64];
    var color: rgb;
    var size: int;
};
```

## Contract

Current Milestone 2 support allows a type to declare that it satisfies a
protocol standard and requires matching receiver-qualified routines.

Current implemented shape:

```fol
std geo: pro = {
    fun area(): int;
};

typ Rect()(geo): rec = {
    var width: int;
};

fun (Rect)area(): int = {
    return 1;
};
```

This subset is intentionally narrow:

- only `pro` is semantic today
- only required routines are semantic today
- the type claim is checked procedurally
- lowering/backend support is not yet implemented, so successful typecheck
  still stops before code generation with an explicit Milestone 2 boundary
- editor hardening now covers contract-header hover/definition on checked-in
  standards examples while keeping broader required-routine hover support out
  of the claimed contract

The broader design remains future work. Later milestones may allow a type to
declare that it satisfies richer standards and check required data and
receiver-qualified routines together.

```fol
std geo: pro = {
    fun area(): flt[64];
    fun perim(): flt[64];
};

std rect(geo): rec[] = {
    width: int[64];
    heigh: int[64];
}
```

Under that design, `rect` would need matching receiver-qualified routines such
as:

```fol
fun (rect)area(): flt[64] = { result = self.width + self.heigh }
fun (rect)perim(): flt[64] = { result = 2 * self.width + 2 * self.heigh }
```

The goal is still procedural. A call like `shape.area()` remains sugar for a
receiver-qualified routine call, not an object-owned virtual method.
