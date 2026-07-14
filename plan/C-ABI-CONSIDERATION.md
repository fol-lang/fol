# C ABI Consideration

Last updated: 2026-03-22

## Purpose

This is not an implementation plan. C ABI is a V4 feature.

This document exists so that V2 and V3 language features (generics, standards,
contracts, closures, advanced enums, metaprogramming, ownership, pointers, and
concurrency/runtime semantics) are designed with eventual foreign-boundary work
in mind. Every such feature should have a clear answer to: "how does this lower
to something ABI-safe or interop-safe?"

If a V2 or V3 design choice makes later C ABI or Rust interop impossible or
ugly, that is a problem now, not in V4.

## The core architecture

```
fol source language
  |
  v
lowered Rust model (canonical shapes)
  |
  v
C ABI projection (mostly automatic)
```

The key insight: do not make FFI understand fol. Make the Rust output
boring enough that FFI generation becomes mechanical.

## Three semantic layers

### Layer 1 — fol

The expressive source language. This is where generics, standards,
closures, pattern matching, advanced enums, ownership, and other rich
features live. No FFI constraints here.

### Layer 2 — canonical Rust model

The lowered output. This layer intentionally reduces fol features to a
small set of Rust constructs that are easy to reason about and easy to
export.

This is the critical layer. It already exists as `fol-lower` producing
`LoweredWorkspace`, but it will need to be extended as V2 features
arrive.

### Layer 3 — foreign projection

Mostly mechanical translation from canonical Rust shapes to
foreign-facing surfaces. This is the V4 work.

For C ABI, that means `extern "C"` wrappers.
For Rust interop, that means compiler-modeled Rust imports/exports over the
same canonical lowered shapes.

## Canonical Rust shapes

Every fol feature should eventually lower to one of these shapes. This
is not a type system — it is a classification for codegen output.

### PodStruct

Plain data with known layout. Maps to `#[repr(C)]` struct.

### TaggedUnion

Enum with tag + payload. Maps to `#[repr(C)]` tag integer + union or
struct per variant.

### OpaqueHandle

Complex stateful object behind `Box<T>`. Maps to forward-declared
opaque pointer in C.

### SliceView

Borrowed contiguous data. Maps to `ptr + len` pair.

### OwnedBuffer

Owned contiguous data. Maps to `ptr + len + capacity` with a free
function.

### Callback

Function pointer + context. Maps to `fn_ptr + void* user_data` in C.

### VTableObject

Collection of function pointers for a standard/protocol. Maps to a C
struct of function pointers.

### StatusCode

Result type lowered to integer status code + out parameter.

## How V2 features should lower

This table is the contract that V2 design decisions must respect.

| fol feature      | canonical Rust shape               | eventual C shape              |
|------------------|------------------------------------|-------------------------------|
| generic fn       | monomorphized concrete fn(s)       | exported concrete C fn(s)     |
| generic type     | monomorphized concrete struct(s)   | one `repr(C)` struct per inst |
| standard/protocol| explicit struct + methods          | opaque handle or C vtable     |
| blueprint        | vtable struct                      | C struct of fn pointers       |
| closure          | `(fn_ptr, ctx_ptr)`                | callback + `void* user`       |
| advanced enum    | tagged struct                      | `repr(C)` tag + payload       |
| `any`/union      | tagged union                       | tag + union                   |
| string           | owned buffer wrapper               | `char*` or ptr+len + free     |
| container        | opaque handle or slice view        | handle + iterator or ptr+len  |
| error/result     | tagged result object               | status code + out param       |

## Design constraints for V2 features

When designing a V2 feature, check these rules:

### 1. Every generic must be monomorphizable

If a generic function or type cannot be fully monomorphized at compile
time, it cannot be FFI-exported. This does not mean generics must always
be monomorphized — only that the path must exist.

Do not design generics that require runtime type parameters.

### 2. Every standard/protocol must be representable as a vtable

A standard that cannot be described as a fixed set of function pointers
with known signatures is not FFI-exportable. This is fine for internal
use, but the language should distinguish between standards that cross
the FFI boundary and those that do not.

### 3. Closures must decompose to function pointer + context

A closure that captures state must be representable as a function
pointer and an opaque context pointer. This is the only closure shape
that C can consume.

Do not design closures that require garbage collection or reference
counting to keep captures alive across the FFI boundary.

### 4. Error types must have a finite status representation

A result type that crosses FFI must lower to a status code plus an out
parameter. This means error types used at the boundary should be
enumerable, not arbitrary objects.

Internal error types can be richer. Only the boundary matters.

### 5. Ownership transfer must be explicit

When an object crosses the FFI boundary, ownership must be clear:

- caller owns and must free → return a handle + provide a free function
- callee borrows → accept a pointer, do not store it
- shared → not allowed at the boundary

Do not design ownership patterns where the FFI boundary is ambiguous
about who frees what.

### 6. No trait objects at the boundary

Trait objects (fat pointers with vtable) are Rust-specific. The lowered
model should convert trait objects to explicit vtable structs before
they reach the FFI layer.

## What this means in practice

When implementing a V2 feature:

1. Design the fol-level syntax and semantics freely
2. Before finalizing, answer: "what canonical Rust shape does this lower
   to?"
3. If the answer is not one of the shapes listed above, either:
   - find a lowering that produces one of those shapes, or
   - add a new canonical shape category with a clear C mapping

The fol language should not be constrained by C. But the lowering from
fol to Rust should always produce shapes that C can consume.

## The three user-facing outputs

This architecture gives fol three export surfaces almost for free:

### 1. Native Rust API

Users who consume fol libraries from Rust get the canonical Rust model
directly. This is already the nicest API.

### 2. C ABI

Generated `extern "C"` wrappers around the canonical Rust model. This
is a V4 deliverable.

### 3. Rust interop

Foreign Rust symbols and types should also attach to the canonical Rust model
rather than bypassing it. That keeps FOL semantics centered on one lowered
representation instead of inventing one path for C ABI and another for Rust.

### 4. Other languages

Any language with C FFI (Python, Go, Zig, Swift, etc.) can bind to the
C layer. This requires no additional compiler work — only documentation
and possibly binding generators.

## Relationship to the current pipeline

The existing pipeline already has the right structure:

```
fol-parser → fol-resolver → fol-typecheck → fol-lower → fol-backend
```

The `fol-lower` stage already produces a `LoweredWorkspace` with
routines, blocks, instructions, and type declarations. That is the
natural place where canonical shape classification should eventually
live.

The `fol-backend` stage already emits Rust source. That is the natural
place where C ABI wrappers and Rust interop glue would be generated in V4.

No new crates are needed. The architecture is already correct.

## Cross-compilation and native libraries

This is the part that must not be missed when C ABI support eventually
arrives:

cross-compilation is not only about compiling fol output for another
target. It is also about linking against native artifacts that match
that same target.

If the selected build target is:

- `aarch64-unknown-linux-gnu`

then every native binary input used by the final link step must also be
compatible with that target, including:

- `.a` static libraries
- `.so` shared libraries
- import libraries or platform-specific linker inputs
- any target-specific runtime objects or sysroot-provided native pieces

Having only host-native libraries is not enough.

Example:

- host machine: `x86_64-unknown-linux-gnu`
- selected build target: `aarch64-unknown-linux-gnu`
- available native library: `libfoo.a` built for `x86_64`

That library is not usable for the `aarch64` build. The build must fail.

### Design rule: native artifacts are target-scoped

Any future native library support should treat binary native inputs as
target-specific assets.

That means the build model should eventually support:

- target-aware native search paths
- target-aware native artifact declarations
- target-aware resolution of link inputs
- explicit diagnostics when no compatible native artifact exists for the
  selected target

The system must not resolve "some libfoo.a" without considering target.

### Design rule: target mismatch should fail early

The backend should not rely only on opaque linker failures.

If fol knows:

- the selected target
- the declared native input
- the available target variants for that input

then it should fail before invoking the final link step when the target
does not match.

Good diagnostic shape:

- artifact `app` links native library `foo`
- selected target is `aarch64-unknown-linux-gnu`
- no compatible native binary was provided for that target

This is much better than a generic linker error.

### Design rule: headers and metadata are not enough

For future FFI import or bindgen-style workflows, headers may be
target-agnostic or mostly target-agnostic, but the linked binary
artifacts are not.

So the model should distinguish between:

- interface metadata
  - headers
  - symbol metadata
  - binding declarations
- target-bound binary inputs
  - static libraries
  - shared libraries
  - import libraries

These should not be treated as the same class of resource.

### Design rule: cross-build success requires toolchain plus native assets

A successful future cross-build against C ABI dependencies will require
all of the following:

- a Rust target toolchain that can build the generated Rust code
- a linker/toolchain that can link for the selected target
- target-compatible native libraries for every linked C ABI dependency

Missing any one of these means the build should fail.

### Implication for future build API design

When native linking is added, the API should be designed around
target-keyed native inputs from the start.

That likely means future native attachment definitions will need to
carry one or more of:

- target triple
- platform family
- architecture
- library kind
- search path origin

Exact schema can be decided later, but target-awareness is not optional.

### What this means for the current cross-compilation plan

The current cross-compilation plan can still proceed without native
linking support.

But it should preserve these future requirements:

- target normalization must be centralized
- build outputs must be target-scoped
- non-host execution must be treated separately from build
- host execution and tool launching must remain separate from source-language
  `core` / `memo` / bundled-`std` API legality
- native-link support later must reuse the same target model instead of
  inventing another one

So the right approach is:

1. add target-aware rustc backend builds now
2. add target-aware native artifact resolution later
3. keep both systems on the same target vocabulary

## Summary

- fol stays expressive — no FFI constraints on language design
- lowering produces canonical Rust shapes — the small boring subset
- FFI generation pattern-matches on those shapes — mostly automatic
- V2 features must have a clear lowering path to canonical shapes
- V4 implements the actual C ABI generation and Rust interop boundary work
- cross-target native linking will require target-matched `.a`/`.so`
  inputs, not just target-aware Rust compilation
