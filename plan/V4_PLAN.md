# V4 Interop and Toolchain Boundary Plan

> **Status: the locked H7 smoke handoff passes; final hardening sign-off is
> pending.** The `x86_64-unknown-linux-gnu` FOL smoke integration now passes
> through PARC, LINC, and GERC via its focused Make-owned gate. The full parent
> `../HARDENING.md` repository gate must still pass before the prerequisite is
> declared complete. This does not complete any broader M0-M10 milestone. A
> checked box in this file means a verified shipped result, not parser
> acceptance or an implementation sketch.

V4 is the release in which FOL becomes an intentional participant in foreign
toolchains. It is not "put `extern \"C\"` around the Rust backend." It is the
combination of:

- a stable, target-specific C ABI projection
- C import and export, including a bounded header-import path
- source-level Rust integration without a stable-Rust-ABI claim
- real executable, static-library, shared-library, object, header, and manifest
  outputs
- target-aware native dependency and linker planning
- the narrow raw-pointer, ownership, cleanup, and conversion rules needed at
  those boundaries
- compiler, frontend, diagnostics, formatter, tools, LSP, tree-sitter,
  examples, CI, docs, and book synchronization in every shipped slice

V4 is the last currently named milestone, but it is **not** shorthand for
"every unfinished language idea." Broader V2 expressiveness, generators and
`yield`, a C++ ABI, unrestricted unsafe code, arbitrary Cargo ingestion, and
other items explicitly excluded below remain separate work.

This file is the sole implementation authority for V4 staging. The former
`plan/C-ABI-CONSIDERATION.md` was intentionally retired rather than retained as
a competing plan. The book remains the user-facing language contract. Each
milestone updates the book from "planned" to "shipped" only after that
milestone's real consumer tests pass.

Where an older statement in this file conflicts with `../HARDENING.md`, the
hardening plan wins. In particular, FOL does not own sibling contract copies,
sibling-to-sibling adapters, the native evidence model, the shared native
target vocabulary, or a second raw Rust FFI emitter. Initial certification is
only `x86_64-unknown-linux-gnu`; Apple, Windows, musl, and other architectures
remain promotion candidates until the sibling pipeline has native evidence for
them.


# 1. Definition of Done

The repository may say **"V4 is fully implemented"** only when all required
milestones M0 through M10 are complete and all of these statements are true:

1. A `build.fol` artifact's kind, model, target, optimization, sources,
   generated inputs, ABI exports, native inputs, link order, outputs, and
   install roles survive build evaluation through package preparation,
   frontend routing, backend compilation, reporting, and installation without
   being reconstructed from defaults.
2. Unknown or unsupported explicit targets fail before `rustc`, a C compiler,
   or a linker is launched. They can never silently produce a host artifact
   under a foreign target directory.
3. Executables, test executables, static libraries, shared libraries, and the
   retained object form produce honest target-specific outputs. Library-only
   workspaces never require or synthesize `main`.
4. Native link plans are typed, ordered, target-checked, provenance-carrying,
   and operational. Local, dependency-provided, exact-file, system-library,
   framework, and import-library inputs either really reach the linker or are
   rejected before it.
5. ABI-relevant integer width and sign, float width, character representation,
   record field order, and entry tag order survive parser -> typecheck -> lower.
6. One compiler-owned foreign surface is the only source for wrappers, C
   headers, Rust facades, manifests, symbol allowlists, and ABI diagnostics.
   The backend never infers public ABI from ordinary internal Rust types.
7. A real C program consumes a FOL static library and a FOL shared library; a
   real FOL program calls a C library; both directions exercise values, errors,
   and ownership rather than only compiling empty stubs.
8. A separate Rust consumer compiles the generated Rust source facade, and a
   FOL consumer calls a Rust adapter through the selected source/bridge model.
   Documentation never promises a stable Rust binary ABI.
9. Every promised ABI type has normative layout, validity, ownership,
   nullability, cleanup, and error rules plus target-specific layout tests.
10. No panic or foreign unwind crosses the C boundary. All boundary inputs are
    validated before they become Rust references, slices, booleans, characters,
    enums, or owned values.
11. Public symbols are explicit and stable. They never contain lowered IDs,
    source traversal order, file order, internal Rust module names, or compiler
    implementation details. The actual exported-symbol set exactly matches the
    manifest allowlist.
12. ABI snapshots distinguish compatible additions from breaking changes, and
    an ABI-breaking change cannot pass without an intentional ABI-major bump.
13. The shipped host/platform matrix passes real compile, link, inspect, and
    run tests as specified in Section 17. Optional lanes cannot make a required
    lane pass vacuously.
14. V4 behavior is mirrored through structured diagnostics and explanations,
    frontend artifact summaries, formatter/tool commands, LSP, tree-sitter,
    positive and failure inventories, docs, examples, and the book in the same
    milestone that exposes it.
15. `make test`, `make tree-test`, `make docs TYPE=mdbook`, the required V4
    native-consumer target, and the ABI compatibility target are green from a
    clean checkout.
16. C header intake, native evidence, and C-to-Rust projection demonstrably
    pass through the locked sibling `../parc`, `../linc`, and `../gerc`
    pipeline. FOL contains no copied or parallel C parser, binary inspector,
    ABI probe engine, provider matcher, or C-to-Rust binding generator.

Anything less is a partial milestone and must be described as such.


# 2. Permanent Guardrails

Keep these rules for all V4 work:

- **C is the durable binary boundary.** It is target-specific, versioned, and
  described by a manifest. Default Rust layout and the Rust ABI are never
  public contracts.
- **Rust integration is source/toolchain integration.** Rust consumers compile
  generated source with a compatible toolchain. FOL consumes Rust through an
  explicitly generated adapter using the canonical foreign model.
- **V4 is not a new `fol_model`.** `core`, `memo`, and the explicit bundled
  `std` dependency remain the capability model. Foreign effects are checked
  against that model; linking a native library never upgrades it.
- **One semantic model, many projections.** Typecheck/lowering own legality and
  canonical ABI shapes. C headers, C wrappers, Rust facades, manifests, docs,
  and editor information all consume that truth.
- **Reuse the sibling interop stack.** `../parc` owns C preprocessing, parsing,
  and source extraction; `../linc` owns native/binary/link/ABI evidence;
  `../gerc` owns C-to-Rust projection and emission. FOL owns orchestration,
  FOL-specific policy, annotations, semantic adaptation, and final artifact
  materialization. Missing facts are fixed in the owning sibling repository,
  never reimplemented locally as a workaround.
- **Internal and public layouts remain separate.** Ordinary FOL records,
  entries, strings, containers, optionals, recoverables, pointers, and runtime
  helpers may keep evolving. Boundary wrappers project them into explicit ABI
  shapes.
- **No stringly linker DSL.** Build records use typed library, framework,
  object, search-path, target, and mode fields. Initial V4 exposes no arbitrary
  linker fragments, linker scripts, or unvalidated response files.
- **No implicit exports.** `[exp]` means FOL package visibility only. An
  artifact owns an explicit ABI export allowlist and exact external names.
- **No implicit ownership.** Every borrowed view, transferred value, owned
  buffer, opaque handle, callback context, and destructor pair says who owns it
  before, during, and after a call.
- **No universal free.** Allocator and destructor provenance are inseparable.
  An arbitrary raw address can never be passed to a generic deallocator.
- **No foreign unwind.** Rust panic, FOL panic, C++ exception, `longjmp`, and
  foreign unwind behavior do not cross a plain C ABI boundary.
- **No accidental host fallback.** Target mismatch is a compiler/build error,
  not a late linker surprise and never a host build with a foreign label.
- **No partial editor story.** Syntax, declarations, names, scopes, intrinsics,
  build fields, model availability, or diagnostics that change in a milestone
  are audited in `fol-editor` and tree-sitter in that same milestone.
- **No compatibility layer.** When V4 chooses a spelling or build API, delete
  the superseded placeholder/parallel route. Do not keep aliases, fallback
  parsing, old projections, or dual production build drivers.
- **No documentation by aspiration.** User docs say a capability ships only
  after a real external consumer proves it.

If a milestone violates one of these rules, stop rather than patch around it.


# 3. Verified Pre-Implementation Truth Snapshot

This section records why the order in this plan is non-negotiable. It describes
the repository at the planning anchor, not the desired V4 result.

## 3.1 Parser, typecheck, and lowering

- `lang/compiler/fol-parser/src/ast/types.rs` preserves integer size/sign,
  float size, character encoding, and `PointerQualifier::Raw`.
- `lang/compiler/fol-typecheck/src/decls.rs::lower_type_inner` collapses those
  scalar variants into unsized `BuiltinType::{Int, Float, Char}`.
- `lang/compiler/fol-typecheck/src/types.rs::CheckedType::Pointer` retains only
  `target` and a `shared` boolean. It cannot represent raw-pointer constness,
  nullability, ownership, escape, or destructor provenance.
- Record declaration order exists in
  `fol_typecheck::RecordFieldLayout`, but
  `lang/compiler/fol-lower/src/decls/type_decls.rs` iterates the checked map
  instead. Entry variants have no equivalent source-order table; current tests
  demonstrate map-order reordering.
- `lang/compiler/fol-parser/src/ast/node.rs`,
  `lang/compiler/fol-typecheck/src/types.rs`, and
  `lang/compiler/fol-lower/src/model.rs` have no foreign declaration, calling
  convention, external name, ownership/effect, ABI import/export, or canonical
  ABI type model.
- `LoweredInstrKind::Cast` and a backend Rust `as` renderer exist for manually
  constructed IR, but source `as` and `cast` are rejected in typecheck and do
  not form an end-to-end conversion contract.
- `ptr[raw, T]` parses and is highlighted, but typecheck correctly rejects it as
  a V4 boundary.
- `.de_alloc(...)` is reserved as a rejected V4 intrinsic without an allocator
  or destructor provenance model.

These facts make scalar preservation and a canonical foreign model blockers.
Generating wrappers from today's `LoweredType` would silently invent an ABI.

## 3.2 Build graph and package flow

- `lang/execution/fol-build/src/graph.rs` can record executable, static,
  shared, object, library-path, system-library, and link metadata.
- `lang/execution/fol-build/src/artifact.rs::project_graph_artifacts` loses or
  fabricates important fields: object is projected incorrectly, target/model
  information is reset, and native attachments are emptied.
- `ExecArtifact`, graph artifacts, projected artifact definitions, package
  native surfaces, frontend selections, and backend config are separate partial
  representations. No single resolved artifact plan survives end to end.
- Dependency artifact handles do not carry an exact output path, resolved
  target, content identity, provenance, or transitive link interface.
- Link cycles, self-links, kind mismatches, target/object-format mismatches,
  duplicate outputs, and static link ordering are not validated.
- Generated-file/tool/install declarations are mostly graph/report metadata;
  there is no general action materializer that owns declared inputs/outputs,
  atomic publication, and actual installation.
- `PreparedPackage::native_surfaces` can retain a manually supplied set, but
  the normal route does not populate an operational native plan.

V4 must fix this foundation rather than attach headers and libraries through a
second backend-only side path.

## 3.3 Backend and frontend

- `lang/execution/fol-backend/src/model.rs::BackendArtifact` has only
  `RustSourceCrate` and `CompiledBinary`.
- `lang/execution/fol-backend/src/emit/skeleton.rs` always generates
  `src/main.rs` and selects an entry routine.
- `lang/execution/fol-backend/src/emit/build.rs` drives `rustc` directly for a
  binary, passes only the internal runtime dependency, and has no native object,
  library, framework, import-library, rpath, or sysroot plan.
- An unknown explicit target currently follows the same `None` path as `Host`,
  so rustc may omit `--target` while the output directory still carries the
  unrecognized target label.
- Internal backend mangling in `lang/execution/fol-backend/src/mangle.rs`
  includes lowered numeric IDs. It is useful internally and categorically
  unsuitable for public ABI symbols.
- `lang/tooling/fol-frontend/src/result.rs::FrontendArtifactKind` cannot report
  static/shared/object/header/import-library/ABI-manifest/Rust-facade outputs.
  Compile routing expects a binary.

## 3.4 Runtime and ABI safety

- `fol_runtime::value::FolInt` is currently `i64`; parsed scalar widths do not
  change that runtime mapping.
- Runtime `bool`, `char`, `String`, `Vec`, `Option`, `FolRecover`, `Rc`, channel,
  mutex, and eventual representations are implementation types, not public C
  layouts.
- The current recoverable ABI is an internal Rust tagged result object. It is
  not the C status/out contract specified below.
- No wrapper currently validates C-originating boolean, Unicode, enum tag,
  pointer, alignment, length, capacity, UTF-8, or output-pointer values.
- No boundary-wide panic containment rule exists.

## 3.5 Tooling, CI, and documentation

- Tree-sitter already recognizes `as`, `cast`, and raw pointer syntax, but
  there is no shipped V4 semantic surface, corpus inventory, or foreign-symbol
  query contract.
- The LSP reuses compiler analysis, but foreign hover/navigation, external-name
  rename restrictions, ABI diagnostics, manifest navigation, and complete V4
  build-record completion do not exist.
- `.github/workflows/tests.yml` is Ubuntu-only and does not establish a
  multi-platform C/Rust consumer matrix. Release output is Linux-musl-only.
- `flake.nix` does not make a pinned C compiler/preprocessor or the required
  sibling interop revisions an explicit V4 toolchain contract.
- `book/src/055_build/600_artifacts.md` and related build chapters describe
  static/shared output and transitive linking more strongly than the current
  executable pipeline implements; `book/src/055_build/900_direction.md` still
  describes missing native-link work.
- The retired C-ABI consideration document contained stale and overbroad
  assumptions; this plan is the only current V4 authority.

## 3.6 Required sibling interop stack

The project owner has selected the sibling checkout stack below. These are
required architecture, not optional inspiration:

| stage | required checkout | revision authority | current crate/schema | owned responsibility |
|---|---|---|---|---|
| PARC | `../parc` | root `interop.lock.toml` | `follang-parc 0.16.0`, source schema 2 | explicit-target C preprocessing, parsing, recovery, source extraction, provenance |
| LINC | `../linc` | root `interop.lock.toml` | `follang-linc 0.1.0`, link-analysis schema 2 | object/archive/shared-library inspection, ABI probes, strict symbol/provider validation, ordered resolved-link evidence |
| GERC | `../gerc` | root `interop.lock.toml` | `follang-gerc 0.1.0`, typed generation domain 1 | closed-world gating, C-to-Rust projection, deterministic raw Rust files and typed link arguments |

The checked-in lock manifest is the machine authority for exact accepted
commits. H7 mirrors that snapshot in CI and the interop book page, and the
Make-owned lock gate rejects drift between them. Every intentional sibling
update changes the lock and its compatibility evidence together.

The hardening prerequisite freezes these rules for V4:

- PARC supplies an explicit-target, fingerprinted `CompleteSourcePackage`;
  partial or rejected scans cannot enter strict integration
- LINC consumes that typed PARC package directly and supplies actual provider,
  layout, symbol, probe, and ordered link evidence through
  `ValidatedLinkAnalysis`
- GERC consumes the typed PARC and LINC checked states directly through
  `GenerationRequest` and returns deterministic raw files plus lossless typed
  link arguments
- no JSON structural adapter, copied domain model, filename-only resolver,
  string link-argument parser, or ambient host fallback is allowed
- only the certified Linux/GNU/ELF lane may be advertised; every other target
  stays rejected or experimental until separately promoted

Therefore V4 must integrate these crates honestly, strengthen upstream
contracts where necessary, and preserve their conservative-rejection model.
It must not paste their code into FOL, hide their diagnostics, infer missing
target facts, or claim broader platform coverage than the locked revisions and
real corpus tests prove.


# 4. Frozen V4 Architecture

These are the chosen defaults. M0 turns them into normative fixtures and docs;
it is not a license to reopen all of them during implementation. A materially
different decision requires an explicit edit to this plan before code lands.

## 4.1 Canonical data flow

There are two directions, but only one canonical FOL ABI model and one selected
C-intake pipeline.

FOL export flow:

```text
FOL source
    -> resolver/typecheck foreign metadata and capability checks
    -> fol-lower constructs fol-abi ForeignInterface + AbiTypeTable
    -> target resolution and ABI verification
    -> one ResolvedAbiSurface
         -> Rust wrapper emission
         -> C header emission
         -> ABI manifest and symbol allowlist
         -> frontend/editor descriptions
    -> compile/link native artifact
    -> PARC re-reads the installed C header
    -> LINC measures layout and inspects/validates the built native artifact
    -> GERC emits the raw Rust binding module from that verified C surface
    -> FOL emits the safe Rust source facade around the GERC module
```

C or Rust-provider import flow:

```text
ResolvedTarget + explicit headers/provider/defines/sysroot
    -> PARC CompleteSourcePackage
    -> LINC AnalysisRequest over that exact typed package
    -> LINC ValidatedLinkAnalysis + probes + inventories + resolved plan
    -> FOL ownership/effect/selection annotation overlay
    -> fol-abi ImportedForeignInterface
    -> GERC GenerationRequest borrowing the same PARC/LINC checked states
    -> GERC GenerationBundle with deterministic private Rust files and typed argv
    -> package/resolver/typecheck safe foreign namespace
    -> fol-lower import operations
    -> typed fol-build link plan + fol-backend generated-source integration
```

A Rust provider first exposes an adapter-owned C header and native library;
from that point it follows the identical import flow. There is no separate
Rust-binary ingestion path.

Implementation and repository ownership:

- `fol-types` owns FOL's user-facing target selection and artifact routing. The
  native interop truth is PARC's checked `TargetSpec` and target fingerprint;
  FOL projects into it once and verifies equality at every sibling boundary
  rather than redefining a shared sibling vocabulary.
- Add `lang/compiler/fol-abi` as a dependency-foundation crate. It owns the
  serializable `ForeignInterface`, `AbiTypeTable`, manifests, schema checks,
  canonical encoding, fingerprints, and compatibility comparison. It may
  depend on `fol-types`, but not parser, package, resolver, typecheck, lower,
  backend, build, frontend, editor, PARC, LINC, or GERC. This foundation is
  necessary because package/resolver import happens before typecheck/lowering,
  while both import tooling and the backend must consume the same schema.
- Add `lang/tooling/fol-interop` as the only cross-repository integration
  layer. It depends on `fol-abi`, `fol-types`, `fol-diagnostics`, `parc`,
  `linc`, and `gerc`; owns orchestration, target-consistency checks, the
  annotation overlay, the pipeline report, and handoff into the FOL action
  graph. It contains no sibling-to-sibling structural adapter and no FOL
  parser/typechecker/backend rules.
- `../parc` remains the only C preprocessing/parser/source-extraction engine.
- `../linc` remains the only direct native inspection, ABI-probe,
  declaration/provider-validation, and provider-evidence engine.
- `../gerc` remains the only general C-to-Rust FFI projection/emission engine.
  FOL may add safe FOL-specific wrappers around its output but must not emit a
  second raw `extern "C"` import module.
- `fol-package` and `fol-resolver` load a checked `fol-abi` import interface
  and synthesize the ordinary foreign namespace; they never parse headers.
- `fol-typecheck` owns source legality, foreign effects, raw-address token
  legality, imported-call eligibility, and the FOL-type-to-ABI decision.
- `fol-lower` constructs or consumes `fol-abi` interfaces, owns source maps and
  boundary operations, and verifies that lowered ABI use matches the checked
  interface.
- `fol-backend` owns FOL export wrappers, C header rendering, export controls,
  safe Rust facade wrappers, incorporation of GERC-generated private modules,
  and compiler invocation from the already verified surface.
- `fol-build` owns resolved artifact/action/link/install plans, not language
  type meaning. LINC link evidence is an input to—not a substitute for—the
  final typed FOL link plan.
- `fol-frontend` routes and reports all produced roles without reinterpreting
  them.
- `fol-editor` consumes compiler/build truth; it does not invent a second ABI
  classifier.

Every declaration receives a stable source identity based on canonical source
origin, range, declaration kind, and C spelling, not vector position. A single
`InteropPipelineReport` records its PARC status, LINC evidence/validation,
annotation decision, FOL ABI identity, and GERC disposition. Any declaration
that is partial, unsupported, ambiguous, unmeasured where measurement is
required, rejected by GERC, or missing from the provider is uncallable. No
stage may silently drop it and still report a fully accepted interface.

Headers, wrappers, manifests, GERC projection, and safe facade generated from
separate or uncorrelated models are a release blocker. Export verification
must round-trip the installed header and built library through
PARC -> LINC -> GERC and compare the normalized public surface back to the
original `fol-abi` interface.

Lowering is target-independent. A `LoweredWorkspace` carries an
`ForeignInterfaceTemplate` containing source widths/encodings, declaration
order, semantic types, boundary operations, and source maps, but no host-sized
offsets, C data-model guesses, output names, or selected artifact exports. For
each `ResolvedArtifactPlan`, `fol-abi` resolves a separate
`ResolvedAbiSurface` from that template plus the plan's exact target, artifact
export/import allowlists, ABI version, and panic policy. A mixed-target or
mixed-profile build therefore produces distinct surfaces/manifests and never
caches one host-resolved layout on the workspace. Imported interfaces are
target-stamped and cannot be reused by a different artifact target.

## 4.2 Capability model and foreign effects

There remains no `fol_model = "std"` and no `fol_model = "ffi"`.

Every imported foreign routine carries a checked effect summary:

- `core`: no allocation, no retained pointers, and no hosted service
- `memo`: may allocate or return/consume owned heap-backed resources
- `std`: hosted IO, filesystem, network, process, thread, blocking, or callback
  behavior; requires the explicit bundled `standard` dependency

Additional orthogonal flags record `may_block`, `may_reenter`,
`may_retain_pointer`, and callback behavior. Unknown header-import behavior is
conservative and is not callable from safe ordinary FOL until an explicit
adapter annotation supplies a supported contract. A native link never widens
the artifact's capability tier.

This preserves the existing honesty boundary: `core` is a FOL source/API
capability contract, not yet a promise that the Rust-produced library is a
freestanding, libc-free, or Rust-`no_std` binary. V4 must not use C linkage to
silently strengthen that claim.

## 4.3 Artifact and output model

One `ResolvedArtifactPlan` is produced once after build evaluation. It carries:

- stable artifact/package identity
- exact artifact kind: executable, test executable, static library, shared
  library, or object
- root sources/modules and generated inputs
- public `fol_model` plus effective runtime tier
- validated target and optimization/profile
- explicit ABI exports and imports
- typed ordered link plan and link-interface propagation
- role-tagged output plan
- install plan
- provenance and cache/build fingerprints

A produced artifact is a set of role-tagged outputs, not one path. Roles cover:

- executable or test executable
- static archive
- shared runtime library
- import library only after a Windows sibling lane is promoted
- relocatable object plus required link-interface sidecar
- C header
- ABI manifest
- native link-interface manifest
- Rust source crate/facade
- debug symbols where the target produces them

`Object` remains in V4 because the version direction names native objects, but
it may ship only with a sidecar that enumerates every runtime/native requirement
the final link still needs. It must never map to a test bundle. If that contract
cannot be implemented, the object constructor is removed rather than faked.

## 4.4 Target authority

Add one fallible FOL routing `ResolvedTarget` in `fol-types`. It records:

- canonical target triple
- architecture, vendor, OS, and environment/ABI
- object format
- pointer width and endianness
- FOL executable/archive/shared naming rules for certified targets
- target support tier

It is not the sibling-native target vocabulary. Interop requires an explicit
PARC `TargetSpec` containing compiler, C data-model, sysroot, dialect, and ABI
facts. `fol-interop` checks that its triple/architecture/OS/width/endianness
agree with the selected FOL artifact and then preserves the PARC target
fingerprint unchanged through LINC and GERC. FOL never fills missing native
facts from its own defaults.

`Host` is explicit and resolved once. An unrecognized explicit target is an
error, never `Host`. Build options, frontend CLI overrides, backend rustc
arguments, header import, output naming, manifests, and editor completion all
consume this type. Delete the current independent target alias tables.

Target precedence is fixed:

1. explicit CLI target override, when the command supports one
2. artifact target from evaluated `build.fol`
3. resolved host target

The selected target controls both FOL runtime compilation and every native
input. Run/test still requires a compatible host or explicit runner; bundled
`std` never grants execution permission.

## 4.5 Native link semantics

`artifact.link(...)` means a binary link dependency, never source reuse. Source
reuse remains `artifact.import(module)`/ordinary package import.

The resolved link plan supports only typed references:

- local produced artifact
- dependency-produced artifact
- exact object/static/shared file (and import libraries only on a promoted lane)
- system library with static/dynamic mode
- framework only on a promoted Apple lane

Rules:

- all native/system requirements currently propagate transitively, matching the
  existing book promise; a later public/private/interface feature is separate
- dependents precede static providers; explicitly declared siblings retain
  declaration order
- ordered inputs and meaningful repetitions are preserved exactly; neither FOL
  nor the backend silently deduplicates LINC/GERC atoms
- self-links and cycles are rejected; V4 does not invent linker groups
- the same source/module cannot be both compiled into a consumer and linked as
  a separately built local artifact if that would duplicate definitions
- frameworks reject until an Apple lane is promoted
- import libraries reject until a distinct MSVC or MinGW lane is promoted
- no default rpath is injected; a future rpath feature must be typed and
  platform-specific
- every exact native binary is inspected for target/object-format compatibility
  before the external linker runs

## 4.6 Stable C ABI type vocabulary

The initial public ABI accepts only explicit, versioned shapes. Internal Rust
or FOL representations never cross directly.

| FOL/canonical shape | C projection | V4 rule |
|---|---|---|
| `int[8/16/32/64]` | `intN_t` | signed width must survive all compiler stages |
| `int[u8/u16/u32/u64]` | `uintN_t` | unsigned width must survive all compiler stages |
| `flt[32/64]` | `float` / `double` | IEEE target support is verified |
| `bol` | `uint8_t` / `fol_bool_t` | only 0 and 1 are valid; imports validate |
| `chr[utf32]` | `uint32_t` / `fol_char_t` | imports validate Unicode scalar values |
| no-value return | no success out field | wrapper still returns status |
| named POD record | generated C struct | source field order, target layout, no hidden fields |
| named entry | explicit tag plus payload union | fixed tag width and explicit stable discriminants |
| borrowed string | `{const uint8_t *ptr; size_t len;}` | UTF-8, call-scoped, never retained unless stated |
| borrowed slice | `{const/mut T *ptr; size_t len;}` | null/len/alignment/overflow validated |
| owned buffer | explicit buffer record plus paired destroy symbol | allocator domain and one-shot transfer are recorded |
| opaque state | forward-declared handle | only generated/imported operations inspect it |
| recoverable result | `fol_status_t` plus success/error out parameters | internal `FolRecover` never crosses |
| callback | function pointer plus context | synchronous, non-retained, same-thread only in V4 |

Boundary restrictions:

- unsized/default `int`, default `flt`, `int[128]`, architecture-sized numeric
  types, non-UTF32 character encodings, and platform C `long` are rejected until
  explicitly added to this matrix
- direct arrays as parameters/returns, packed records, bitfields, flexible
  arrays, arbitrary unions, C varargs, vector/SIMD ABI, and complex numbers are
  rejected
- generic routines/types must be wrapped in an ordinary non-generic FOL
  routine/type before export; no implicit generic ABI export
- `str`, `vec`, `seq`, `set`, `map`, `opt`, errors, owned values, borrows,
  unique/shared pointers, `Rc`, standards, channels, eventuals, mutexes, and
  routine objects require an explicit canonical projection above or reject
- record/entry names and fields used in ABI must be named public declarations;
  anonymous structural aggregates do not become public ABI identities

The first scalar milestone does not wait for every aggregate, but no aggregate
ships until its row has layout and lifetime tests.

## 4.7 Uniform status and panic contract

Every exported C function returns a fixed signed 32-bit `fol_status_t`.
Ordinary results are written through out parameters. This uniformity prevents
infallible-looking functions from lacking a panic/validation channel.

Reserved status values:

- `0`: success
- `1`: FOL recoverable report; the typed error out parameter is initialized
- `-1`: invalid foreign argument (null, tag, boolean, Unicode, length, etc.)
- `-2`: contained FOL/Rust panic
- `-3`: internal wrapper/runtime failure

User-defined error payloads are separate ABI-safe out values; they are not
encoded into ad hoc status numbers. On failure, success out values remain
uninitialized and wrappers must not read or drop them. The generated header
documents this rule.

Generated library wrappers use the selected explicit panic strategy. The V4
default is catch-and-translate at every exported wrapper. If a target/toolchain
cannot support that strategy, the artifact fails to build rather than silently
changing the ABI to unwind. Foreign C/C++ exceptions and `longjmp` remain
unsupported. Plain `extern "C"` never carries an unwind.

## 4.8 Raw pointers, ownership, and cleanup

Raw pointers are non-owning foreign address tokens, not aliases for V3 `Box` or
`Rc` pointers.

Chosen source shape:

- `ptr[raw, T]`: non-null read-only raw address token
- `ptr[raw, mut, T]`: non-null mutable raw address token
- `opt ptr[raw, T]` / `opt ptr[raw, mut, T]`: nullable forms

The parser/type system must preserve raw-ness and mutability rather than
collapsing them to `shared: bool`. Ownership, escape, and destructor provenance
are signature/manifest metadata, not guessed from the address.

V4 permits a raw address token to be received, compared for identity, stored
only where its lifetime contract allows, and passed to an approved foreign
adapter. V4 does **not** permit:

- ordinary raw dereference
- pointer arithmetic
- integer-to-pointer or pointer-to-integer conversion
- constructing raw pointers from arbitrary FOL references
- changing constness/nullability/ownership with a cast
- sending raw pointers across tasks/threads without a later explicit contract

There is no general unsafe block in V4. Imported declarations that cannot be
projected into the safe canonical shapes stay uncallable and require a small C
or Rust adapter.

Delete the placeholder `.de_alloc(...)` intrinsic during M0/M4. It must not
become a universal free. Owned resources instead expose a type/provider-specific
destroy or release routine recorded in the manifest. The ownership checker
consumes the resource exactly once at that call and diagnoses wrong-provider,
double-release, use-after-release, missing-release, and borrowed-release cases.

## 4.9 Explicit conversion

V4 does not turn the existing generic Rust `as` renderer into a transmute
facility.

- `as` becomes the single source spelling for explicitly lossless fixed-width
  numeric conversion.
- The duplicate `cast` keyword/operator/intrinsic/editor spelling is deleted,
  with no alias or compatibility route.
- Narrowing, float/integer conversion, bool/integer conversion, character
  conversion, pointer conversion, ownership conversion, bit reinterpretation,
  and container conversion remain rejected unless a later plan gives them a
  checked result contract.
- ABI boolean/character/tag/pointer validation uses dedicated typed lowering
  operations generated by the ABI projection, not user-visible `as`.

This keeps V4's conversion work sufficient for ABI projection without silently
claiming a complete general casting system.

## 4.10 ABI export identity

`[exp]` is necessary for a source declaration to be selected, but it is never
sufficient to export a native symbol. Each library artifact declares one ABI
major/minor version and carries explicit allowlist entries with:

- fully qualified FOL routine
- exact external C symbol
- optional stable Rust facade name

M0 freezes one build API spelling; the planned canonical spelling is an
artifact method shaped as:

```fol
library.set_abi_version({ major = 1, minor = 0 });
library.add_abi_export({
    routine = "api::add",
    symbol = "fol_demo_add",
    rust_name = "add",
});
```

If the evaluator requires a different record spelling, update this plan and all
normative fixtures first; do not ship two names.

External names are exact (no backend mangling), ASCII C identifiers, nonempty,
not globally reserved C identifiers, and unique within the final link/export
set. Internal routines remain ID-mangled and private. Export-control files or
linker visibility settings are generated from the allowlist so the shared
library does not leak every `pub fn` in generated Rust.

## 4.11 ABI manifest and compatibility

Every target-specific library emits `<artifact>.folabi.json`. It contains:

- schema identifier and schema version
- artifact/package identity and ABI major/minor
- canonical target and C ABI/calling convention
- panic policy
- exports and imports with external names
- canonical type graph
- field order, offsets, sizes, alignments, tag widths, and discriminants
- parameter direction, mutability, nullability, ownership, escape, and
  destructor pairs
- status/out rules and callback rules
- required native link interface
- compiler/runtime/toolchain identity and native-input provenance

Use deterministic canonical JSON: UTF-8, sorted object keys, semantic arrays in
defined order, no insignificant formatting, and a versioned SHA-256 digest.
Record two different hashes:

- `interface_fingerprint`: only public target ABI facts; this controls ABI
  compatibility
- `build_fingerprint`: compiler/runtime/toolchain/profile/native inputs/link
  order; this controls cache/reproducibility identity

Changing compiler version or an internal routine must not change the interface
fingerprint. Removing/changing a public symbol/type/layout/ownership/error rule
is breaking. Adding a disjoint symbol/type is minor-compatible. A checked-in
baseline comparison fails breaking changes unless the ABI major is explicitly
incremented. Cross-target manifests are never compared as if layout-compatible.

## 4.12 Sibling dependency and compatibility contract

The development topology is a coordinated sibling checkout:

```text
parent/
  fol/
  parc/
  linc/
  gerc/
```

`lang/tooling/fol-interop/Cargo.toml` declares `parc`, `linc`, and `gerc` as
exact path dependencies into the sibling checkout topology; only
`fol-interop` consumes all three. Do not vendor their sources into FOL and do
not add optional copied fallback implementations.

The root `interop.lock.toml` records, for each sibling:

- canonical relative path and canonical remote identity
- full Git commit, crate version, and serialized-contract schema version
- accepted feature set

It also records one digest over the GERC H5 compatibility driver, fixtures,
and support code.

`Cargo.lock` is not sufficient because it does not pin the Git identity of path
dependencies. `make interop-check` verifies paths, schemas, selected features,
typed orchestration compilation, and compatibility fixtures for active
development.
`make interop-locked` additionally requires the exact locked commits and clean
worktrees for CI/release. CI checks out the three repositories as siblings at
those exact commits before invoking the FOL Makefile.

Upgrades are one stage at a time:

1. make the missing semantic fact or API change in its owning sibling
2. run that repository's `make test` and commit there independently
3. update only the corresponding FOL orchestration boundary and fixtures
4. run the whole PARC -> LINC -> GERC compatibility corpus
5. update `interop.lock.toml` and the FOL build fingerprint

Never make an uncommitted multi-repository state the only passing state. Do not
use `[patch]`, a local fork, a copied module, or an unrecorded floating branch to
hide divergence. If FOL is later published as a standalone source crate, the
three upstream crates must first be published/pinned or the release topology
must be explicitly redesigned; V4 does not pretend path-only Cargo dependencies
are a self-contained crates.io package.

## 4.13 C import and header strategy

Do not add a handwritten C parser or a parallel Clang-AST importer to the FOL
compiler. `fol tool bind c` is an orchestration command over the locked
PARC -> LINC -> GERC pipeline.

The command accepts an explicit target, C dialect, preprocessor/compiler,
sysroot, entry headers, user/system include roots, defines, provider artifacts,
and annotation file. These facts construct one checked PARC `TargetSpec`; LINC
and GERC consume its exact target fingerprint through their typed upstream
artifacts rather than receiving separately reconstructed configurations.
Ambient target, `CPATH`, `C_INCLUDE_PATH`, SDK, compiler, or sysroot discovery
is disabled in reproducible mode and is only available through a separately
named, fingerprinted host-discovery mode.

The pipeline is:

1. PARC preprocesses/parses/extracts a target-stamped
   `CompleteSourcePackage`; partial or rejected output stops here.
2. LINC consumes that exact typed source closure, inspects each exact provider,
   measures every required layout, validates source declarations against
   symbols, and returns `ValidatedLinkAnalysis` with an ordered resolved plan.
3. FOL checks that source, target, provider, and evidence fingerprints match,
   then attaches LINC's typed ordered atoms to its action graph without text
   rendering, reparsing, deduplication, or provider substitution.
4. The explicit annotation overlay supplies facts C cannot express: ownership,
   pointer/length pairing, direction, nullability, escape, destructor pairing,
   effects, and the selected callable subset.
5. The accepted result becomes the same `fol-abi::ForeignInterface` used by
   exports and gets a deterministic target-specific manifest.
6. A `gerc::GenerationRequest` borrows the same accepted PARC source and LINC
   evidence. GERC must accept exactly the raw declarations needed by the FOL
   interface and emits the private raw Rust files plus typed Rust link plan.

GERC has no build-script or stringly-link side channel for the private FOL
module. FOL appends every validated `OsString`-equivalent link argument as one
backend process argv item through the typed `fol-build` plan. It never shell
splits, flattens, or reparses GERC output.

The annotation schema is versioned, rejects unknown fields, uses stable
declaration identities rather than source order, and is part of both
fingerprints.

Initial supported header subset:

- ordinary C functions with C/system calling conventions
- fixed-width scalar typedefs
- non-packed POD structs without bitfields/flexible arrays
- opaque forward-declared handle types
- target-resolved enums only when their width and values are explicit in the
  manifest
- pointer shapes only when adapter annotations make their safety contract
  complete

Macros, variadics, C++, overloaded symbols, templates, bitfields, packed
structs, flexible arrays, arbitrary unions, and inline implementation import are
rejected with structured diagnostics. Unsupported declarations may remain in
the header; they simply do not become callable FOL symbols.

The build graph attaches the checked import interface and exact target-matched
native provider. The package/resolver layer synthesizes a foreign namespace;
ordinary FOL source uses normal namespace/import lookup. V4 adds no parallel
handwritten `extern` source declaration grammar.

PARC partial/unsupported diagnostics, LINC unresolved/ambiguous/mismatched
evidence, and GERC rejection are hard acceptance gates for the affected
declaration. Cross-target layout probes require an explicit supported runner or
non-executing evidence path implemented in LINC; they never execute a foreign
probe on the host and never substitute host layout. A missing upstream
capability narrows the supported matrix until fixed in that sibling.

## 4.14 Rust integration boundary

The current production direct-`rustc` path remains canonical for FOL-only and C
ABI artifacts. V4 does not quietly add Cargo as a second production truth.

FOL -> Rust:

- round-trip the installed FOL C header and native artifact through
  PARC -> LINC -> GERC
- use the GERC-emitted raw module as the only unsafe `extern "C"` substrate
- emit a Cargo-compatible safe source facade with `src/lib.rs` around it
- expose stable safe Rust names selected from the same ABI export list
- keep unsafe glue private
- compile with the consumer's compatible toolchain and exact bundled/published
  FOL runtime dependency
- include target, compiler/runtime, feature, and minimum/supported rustc
  metadata

Rust -> FOL:

- generate/scaffold a Rust adapter crate whose public edge is the canonical C
  ABI/manifest
- the adapter's own Cargo build owns its dependency graph, features, lockfile,
  proc macros, and build scripts
- FOL consumes the produced target-matched header/library through the exact
  PARC -> LINC -> GERC import/link path

This proves both directions without claiming arbitrary `.rlib` compatibility.
Prebuilt Rust `rlib`/`dylib` ingestion, automatic Cargo feature resolution by
FOL, and executing third-party build scripts inside `build.fol` are outside V4.
If broad Cargo ingestion is wanted later, it needs a separate plan and must
replace—not coexist ambiguously with—the production driver.

The Rust Reference explicitly gives the Rust ABI no stability guarantee, while
`extern "C"` follows the target's dominant C ABI. Default Rust layout is also
not a public layout contract. Keep those facts linked in docs:

- <https://doc.rust-lang.org/reference/items/external-blocks.html>
- <https://doc.rust-lang.org/reference/type-layout.html>
- <https://doc.rust-lang.org/reference/linkage.html>
- <https://doc.rust-lang.org/rustc/command-line-arguments.html>
- <https://doc.rust-lang.org/nomicon/ffi.html>
- <https://doc.rust-lang.org/nomicon/unwinding.html>


# 5. M0 — Contract Freeze, Characterization, and Truth Repair

M0 lands before semantic or backend V4 code.

Primary files:

- `plan/V4_PLAN.md`
- `ARCHITECTURE.md`
- `book/src/055_build/200_graph_api.md`
- `book/src/055_build/300_handle_api.md`
- `book/src/055_build/600_artifacts.md`
- `book/src/055_build/700_cross_compilation.md`
- `book/src/055_build/900_direction.md`
- new normative fixtures under `examples/v4_contract_*` and
  `examples/fail_v4_contract_*`

Tasks:

- [ ] Re-run the truth snapshot and record changed symbols/files in this plan.
- [ ] Add characterization tests for every currently lossy projection, unknown
  target host fallback, library-to-binary routing, dropped native link input,
  unstable scalar/order fact, and ID-based public-name hazard.
- [ ] Freeze the exact `build.fol` spelling for ABI exports, C imports, native
  exact files, target-specific providers, and adapter annotations. Check in
  parser/evaluator fixtures even though the semantic path still rejects them.
- [ ] Freeze the C header naming, include guard, status values, scalar typedefs,
  manifest schema, canonical JSON, two fingerprints, and install layout.
- [ ] Freeze the platform tiers, required toolchain versions, and skip policy.
- [ ] Freeze the safe-import rule: no general unsafe block, unsupported raw C
  declarations require an adapter.
- [ ] Delete `cast` as a duplicate planned spelling and delete the
  `.de_alloc` placeholder; update diagnostics, intrinsics inventory, lexer,
  parser fixtures, tree-sitter grammar/queries/corpus, book, and examples in
  the same commit.
- [ ] Correct book/architecture claims that static/shared linking already works.
  Restore shipped wording only at the milestone that proves it.
- [ ] Revalidate the retained ABI rationale in this plan against the refreshed
  truth snapshot.
- [ ] Add an ABI diagnostic family to `fol-diagnostics` (planned family label
  `ABI`, with stable producer-owned codes) and reserve exact codes only when a
  real construction site exists.

Verification:

- `make test`
- `make tree-test`
- `make docs TYPE=mdbook`
- `git diff --check`
- Characterization tests must fail if their known bug is accidentally hidden by
  another default; they are converted to positive regression tests in the
  milestone that fixes the bug.

**STOP:** no M1 code starts while any rule in Section 4 is still represented by
two competing spellings, an undocumented backend assumption, or an unresolved
"TBD".


# 6. M1 — One Target and One Resolved Artifact Plan

Goal: eliminate lossy artifact/target reconstruction before adding new outputs.

Primary files and symbols:

- new `lang/compiler/fol-types/src/target.rs`
- `lang/execution/fol-build/src/option.rs`
- `lang/execution/fol-build/src/artifact.rs`
- `lang/execution/fol-build/src/graph.rs`
- `lang/execution/fol-build/src/runtime.rs`
- `lang/execution/fol-build/src/executor/types.rs::ExecArtifact`
- `lang/execution/fol-build/src/executor/graph_methods.rs`
- `lang/execution/fol-build/src/eval/{source.rs,plan.rs,types.rs}`
- `lang/compiler/fol-package/src/{build_dependency.rs,model.rs}`
- `lang/tooling/fol-frontend/src/{config.rs,build_route/mod.rs,compile/mod.rs}`
- `lang/execution/fol-backend/src/config.rs`

Tasks:

- [ ] Add fallible `ResolvedTarget` and delete duplicated target normalization.
- [ ] Reject invalid architecture/OS/environment combinations and unknown
  explicit triples before output directories or commands are created.
- [ ] Define `ResolvedArtifactPlan` with every field listed in Section 4.3.
- [ ] Produce it once from the evaluated graph; remove or completely replace
  `project_graph_artifacts` instead of retaining a compatibility projection.
- [ ] Carry the plan losslessly through package preparation and frontend
  artifact selection into a backend compile plan.
- [ ] Split executable and test-executable identity; preserve object/static/
  shared kinds exactly.
- [ ] Make CLI/artifact/host target precedence explicit and test it.
- [ ] Include artifact kind, target, model/effective tier, profile, inputs,
  exports, link plan, and output roles in deterministic plan identity.
- [ ] Redact/hash selected environment values in determinism data rather than
  persisting secrets verbatim.

Tests:

- mixed-target and mixed-profile artifacts retain independent values
- a library-only graph reaches a library backend plan or gives a precise
  not-yet-supported error; it never reports a binary success
- object remains object through every layer
- unknown explicit target proves no external process launched and no mislabeled
  output appeared
- serialized/equality round-trip covers every ABI-affecting field
- frontend summaries show selected kind/target/model before compilation

Verification:

- `make test`
- `make docs TYPE=mdbook`
- targeted build/front-end tests through `make test TEST_ARGS=<filter>`

**STOP:** M2 cannot begin while any downstream layer fills an ABI-affecting
field from `Default` or guesses it from an output filename.


# 7. M2 — Operational Action Graph and Materializer

Goal: make generated inputs, compilation, installation, and produced outputs
real graph actions so V4 does not grow a backend-only side channel.

Primary files:

- `lang/execution/fol-build/src/graph.rs`
- `lang/execution/fol-build/src/codegen.rs`
- `lang/execution/fol-build/src/step.rs`
- `lang/execution/fol-build/src/api/build_api.rs`
- new `lang/execution/fol-build/src/materialize.rs` (or one equivalently named
  canonical executor module)
- `lang/tooling/fol-frontend/src/build_route/mod.rs`
- `lang/tooling/fol-frontend/src/compile/mod.rs`

Tasks:

- [ ] Store typed action payloads for write, copy, system tool, codegen,
  compile, install, and run operations.
- [ ] Give every action declared inputs, role-tagged outputs, dependencies,
  target, and cache identity.
- [ ] Validate action/step cycles, missing producers, duplicate output paths,
  duplicate install destinations, and output escaping before execution.
- [ ] Canonicalize package/build-relative paths and reject traversal or symlink
  escape from allowed roots.
- [ ] Execute only the requested step closure in deterministic order.
- [ ] Materialize in a per-plan temporary directory with a process lock and
  atomic final publication; parallel builds must not delete each other's
  generated crate/output directories.
- [ ] Treat a successful tool process that omitted a declared output as an
  error.
- [ ] Implement actual install/copy behavior with target-specific roles and
  collision checks.
- [ ] Fingerprint tools and inputs without printing secret environment values.
- [ ] Keep dependency-provided executable tools disabled until a separate trust
  policy exists.

Tests:

- existing generated/write/copy/install examples create their declared files
- missing output, duplicate destination, traversal, and symlink escape fail
- parallel independent actions succeed; colliding actions fail deterministically
- interrupted action never publishes a partial final output
- two clean materializations produce identical manifests/hashes when the
  underlying toolchain claims reproducibility

Verification:

- `make test`
- add a Make-owned action/materializer integration target before declaring M2
  complete

**STOP:** M3 may not attach headers/manifests directly to the backend if the
graph still cannot own, cache, report, and install them.


# 8. M3 — Real Backend Artifact Families and Native Link Plans

Goal: make the artifact kinds already named by `build.fol` operational before
exposing foreign language syntax/semantics.

Primary files:

- `lang/execution/fol-backend/src/{config.rs,model.rs,identity.rs}`
- `lang/execution/fol-backend/src/emit/{skeleton.rs,build.rs,layout.rs,runtime.rs}`
- `lang/execution/fol-build/src/{native.rs,graph.rs,dependency.rs,artifact.rs}`
- `lang/execution/fol-build/src/executor/handle_methods.rs`
- `lang/compiler/fol-package/src/build_dependency.rs`
- `lang/tooling/fol-frontend/src/{compile/mod.rs,result.rs}`

Tasks:

- [ ] Add backend product kinds and role-tagged `ProducedArtifact` output sets.
- [ ] Generate `src/main.rs` only for executable/test products and `src/lib.rs`
  for library/object/Rust-source products.
- [ ] Drive rustc with the correct `bin`, `staticlib`, and `cdylib` crate types;
  implement object output only with its complete link-interface sidecar.
- [ ] Derive all certified output names from `ResolvedTarget`; Windows runtime,
  import-library, and platform debug-symbol roles remain rejected until their
  sibling lanes are promoted.
- [ ] Preflight rustc target availability, linker, archiver, sysroot, C compiler,
  and symbol tools before compilation.
- [ ] Replace `NativePlatform`/synthetic framework strings with target-aware
  typed native inputs.
- [ ] Resolve local, dependency, exact-file, object, system-library, and
  framework handles into one ordered `NativeLinkPlan`.
- [ ] Give dependency artifact exports exact role paths, target, content digest,
  provenance, and transitive link interface.
- [ ] Validate cycles, self-links, incompatible artifact kinds, target/object
  format, missing roles, duplicate symbols/providers where knowable, and
  framework platform.
- [ ] Translate the plan to structured rustc/linker arguments; never concatenate
  user-provided raw flag strings.
- [ ] Include toolchain/native inputs/link order in the build fingerprint and
  isolate output/cache directories by artifact kind and target.
- [ ] Report and install every produced role through the frontend.

Tests:

- host executable, static library, and shared library have the expected file
  format and target
- library-only artifact does not look for `main`
- executable links a local static library without compiling the same source
  twice
- multi-level static closure uses stable dependent-before-provider order
- shared library consumes its direct dependencies
- dependency-exported archive really reaches the link command
- wrong-target object/archive fails before the linker
- system-library positives/negatives are target-specific; Apple framework
  inputs fail early on the initial certified lane
- Windows MSVC and GNU inputs fail early until their separate evidence lanes
  exist and never share a naming/import-library branch
- parallel builds of identical source for different kinds/targets do not collide
- frontend JSON lists static/shared/object/import-library/debug-symbol roles

Verification:

- `make test`
- new mandatory `make test-native`
- `make docs TYPE=mdbook`

**STOP:** no M4 foreign surface is user-visible until a tiny real C program can
link and run against empty/scalar-free static and shared FOL library shells on
the required host lane.


# 9. M4 — Preserve ABI Facts and Add the Canonical Foreign Model

Goal: create compiler truth before wrappers or header generation.

Primary files:

- `lang/compiler/fol-parser/src/ast/{types.rs,node.rs,options.rs}`
- `lang/compiler/fol-resolver/src/{model.rs,traverse/}`
- `lang/compiler/fol-typecheck/src/{types.rs,model.rs,decls.rs,session.rs}`
- new `lang/compiler/fol-typecheck/src/abi.rs`
- `lang/compiler/fol-lower/src/{types.rs,model.rs,session.rs,verify/}`
- new `lang/compiler/fol-lower/src/abi.rs`
- `lang/compiler/fol-diagnostics/src/explain.rs`
- `lang/compiler/fol-intrinsics/src/`

Tasks:

- [ ] Carry integer width/sign, float width, and character encoding into checked
  and lowered type identity without breaking ordinary runtime defaults.
- [ ] Preserve record declaration order through lowering and add entry variant
  order plus explicit stable discriminants.
- [ ] Add raw-pointer checked/lowered variants with raw-ness and mutability;
  optional wrapping remains the nullability marker.
- [ ] Add foreign import/export metadata, external name, calling convention,
  ownership/nullability/escape/destructor facts, effects, and source origin.
- [ ] Add `AbiTypeId`, `AbiTypeTable`, canonical shapes from Section 4.6, and a
  target-resolved `ForeignInterface`/`ResolvedAbiSurface` on `LoweredWorkspace`.
- [ ] Add a verifier that rejects every non-projectable type before backend
  emission and reports the complete path to the offending nested field.
- [ ] Keep package visibility separate from ABI export selection.
- [ ] Add stable ABI diagnostics with primary declaration, related offending
  field/native attachment, note, help, and exact code; register explanations
  only for codes with construction sites.
- [ ] Implement only lossless numeric `as`; remove every remaining `cast`
  spelling and keep pointer/ownership/transmute conversions rejected.
- [ ] Remove `.de_alloc`; model paired destroy routines instead.
- [ ] Serialize the deterministic manifest model and compute separate interface
  and build fingerprints.
- [ ] Add compiler metadata APIs used by LSP/build completion; do not duplicate
  type matrices in editor code.

Required negative classifier cases:

- default/unsized numeric, 128-bit, unsupported character encoding
- generic declaration or generic parameter
- anonymous record/entry
- unordered/unstable entry tag
- internal string/container/optional/error without a projection
- owned/borrowed/unique/shared pointer without a canonical wrapper
- raw pointer missing mutability/nullability/ownership/escape/destructor facts
- standard/protocol object, routine object, closure
- channel, sender, eventual, mutex, task state
- recursive by-value aggregate, packed/bitfield/flexible form
- duplicate/reserved/invalid external symbol
- capability/effect stronger than artifact model

Tests:

- `int[u16]`, `int[32]`, `flt[32]`, and `chr[utf32]` survive every stage
- source record order `z, a` remains `z, a`
- entry order/tags remain stable across file/declaration reorder
- internal lowered IDs change in a fixture without changing public symbol or
  interface fingerprint
- classifier walks nested aggregates and points to the exact bad field
- manifest canonicalization is byte-identical across repeated clean runs
- compiler rejects a foreign surface before backend if any contract is missing

Verification:

- `make test`
- `make tree-test`
- `make docs TYPE=mdbook`

**STOP:** M5 cannot start if scalar facts are erased, aggregate order/tags are
unstable, a public symbol contains an internal ID, or the backend would need to
rediscover ABI legality.


# 10. M5 — Scalar C Export Vertical Slice

Goal: prove the smallest complete FOL -> C path through every layer.

Required slice:

- signed/unsigned 8/16/32/64 integers
- 32/64-bit floats
- ABI boolean and UTF-32 character validation/projection
- no-value and scalar results through the uniform status/out contract
- explicit ABI export allowlist and names
- static and shared libraries
- generated header, manifest, symbol allowlist, install roles

Primary files:

- `lang/compiler/fol-lower/src/abi.rs`
- new `lang/execution/fol-backend/src/abi/`
- `lang/execution/fol-backend/src/emit/{skeleton.rs,build.rs}`
- `lang/execution/fol-runtime/src/abi.rs` (only stable wrapper substrate; do not
  expose existing `FolRecover`)
- `lang/tooling/fol-frontend/src/{compile/mod.rs,result.rs}`
- `examples/v4_c_export_scalar/`
- `test/integration_tests/integration_v4_ffi.rs`

Tasks:

- [ ] Emit private internal Rust functions plus public `extern "C"` wrappers
  using exact allowlisted symbols and target calling convention.
- [ ] Use explicit representation types and `#[repr(C)]` only on generated ABI
  facade records; never blanket-mark internal FOL records.
- [ ] Use the Rust edition's required unsafe-attribute spelling for exported
  names (`#[unsafe(no_mangle)]`/`export_name` where applicable) and pin generated
  edition/toolchain semantics.
- [ ] Validate all inbound scalar bit patterns before conversion.
- [ ] Catch/translate panics and implement the status/out initialization rules.
- [ ] Generate header and manifest from the exact same resolved surface.
- [ ] Generate export controls and inspect the built symbol set.
- [ ] Compile the header as C11 and include it from a C++ translation unit only
  as an `extern "C"` header smoke test; this is not C++ ABI support.
- [ ] Install and consume the artifact from its installed layout, not only its
  build directory.
- [ ] Add frontend human/plain/JSON output for every role.

Real consumer tests:

- C calls each scalar export through static library
- C calls the same API through shared library
- scalar `core` and std-free `memo` libraries build without declaring bundled
  `std`; a hosted export is rejected until the explicit `standard` dependency
  is present
- invalid boolean and Unicode inputs return `-1`
- FOL recoverable report returns `1` and initializes only the error out
- FOL panic returns `-2` and does not unwind into C
- null required out pointer returns `-1`
- exact symbol inspection finds all and only allowlisted exports
- two clean builds produce the same header/manifest/interface fingerprint

Verification:

- `make test`
- `make test-native`
- new `make test-v4-c`
- new `make abi-check`

**STOP:** do not call this slice shipped if the C test uses Rust/FOL internals,
skips real linking, bypasses installation, or asserts only generated text.


# 11. M6 — C Imports and Native Providers

Goal: prove the opposite direction using the same foreign model and real native
link plan.

Primary files:

- `lang/compiler/fol-package/src/{model.rs,metadata.rs,build_dependency.rs}`
- `lang/compiler/fol-resolver/src/{model.rs,imports.rs}`
- `lang/compiler/fol-typecheck/src/{session.rs,abi.rs}`
- `lang/compiler/fol-lower/src/{abi.rs,control.rs,exprs/}`
- `lang/execution/fol-build/src/{native.rs,dependency.rs}`
- `lang/execution/fol-backend/src/{signatures.rs,instructions/}`
- `lang/tooling/fol-frontend/src/build_route/`
- `examples/v4_c_import_scalar/`

Tasks:

- [ ] Load a checked ABI manifest into a synthetic foreign namespace with
  stable source/header origins for diagnostics/navigation.
- [ ] Add `ForeignCall` IR distinct from internal `LoweredRoutineId` calls.
- [ ] Require LINC's checked analysis to resolve every imported symbol to
  exactly one target-compatible provider and ordered link role before
  lowering/backend execution; FOL verifies and carries that result but does not
  run another resolver.
- [ ] Compile GERC's typed unsafe extern module as the only raw import layer;
  FOL-generated safe adapters own language policy, validation, and capability
  checks without re-emitting the extern declaration.
- [ ] Enforce foreign effects against `core`/`memo`/effective `std` without
  implicit upgrades.
- [ ] Keep unknown/unsafe raw declarations uncallable.
- [ ] Prove local exact-file, dependency-provided archive, dynamic library,
  system library, and target-specific missing-provider diagnostics.
- [ ] Report missing symbol/library, wrong architecture/object format, duplicate
  provider, and link cycle before or with structured related-site diagnostics;
  do not expose a raw linker dump as the primary error.

Real consumer tests:

- FOL calls a checked C scalar library in static and shared form
- multi-level native static dependency order works
- a dependency package exports a real native artifact and interface
- `core` accepts a declared core-safe scalar call but rejects allocation/hosted
  effects
- `memo` and explicit bundled `std` gates behave independently of linking
- wrong target, wrong format, missing symbol, duplicate provider, and unsafe
  declaration fail at the earliest owned stage

Verification:

- `make test`
- `make test-native`
- `make test-v4-c`
- `make abi-check`

**STOP:** an imported function cannot ship if its provider path, target,
provenance, effect, calling convention, or safety contract is unknown.


# 12. M7 — Records, Entries, Errors, Views, Buffers, and Handles

This milestone is a sequence of independently gated sub-slices. Land them in
the listed order; omit a later slice rather than weakening an earlier contract.

## 12.1 POD records

- [ ] Project only named ABI-safe records into generated C/Rust facade structs.
- [ ] Preserve source field order; compute and record target size/alignment/
  offset/padding.
- [ ] Generate C `_Static_assert` and Rust const/layout probes.
- [ ] Reject recursion by value, hidden runtime fields, packing, and unsupported
  nested types.

Gate: C and Rust probes agree for the certified target.

## 12.2 Entries and recoverable errors

- [ ] Use a fixed explicit tag type and stable numeric discriminants.
- [ ] Use a generated payload union only for projectable payloads.
- [ ] Keep the universal status value separate from user error payload/tag.
- [ ] Reject unknown tags before constructing an internal entry.

Gate: declaration reorder does not silently renumber tags; an intentional tag
change is an ABI break.

## 12.3 Borrowed strings and slices

- [ ] Validate null/zero-length combinations, alignment, `len * size` overflow,
  `isize` bounds, mutability, aliasing, and UTF-8 before constructing Rust views.
- [ ] Limit default lifetime to the call; `may_retain_pointer` requires a
  different owned/handle projection and cannot use a borrowed view.
- [ ] Forbid concurrent mutable aliases and callback retention.

Gate: null, zero-length, misaligned, overflow, invalid UTF-8, retained-view, and
mutability negatives pass under sanitizers.

## 12.4 Owned buffers and opaque handles

- [ ] Record ownership direction and exact generated/imported destroy symbol.
- [ ] Give every allocator/provider domain a stable identity checked by the
  destroy adapter.
- [ ] Consume transferred handles exactly once in ownership checking.
- [ ] Generate FOL-owned C destroy wrappers and honor imported C destroy pairs.
- [ ] Diagnose leak paths where an owned foreign resource exits scope without
  transfer or destruction; use explicit cleanup/`dfr`, not a universal free.
- [ ] Validate capacity/length/domain before reconstructing owned buffers.

Gate: create/use/destroy, early-error cleanup, wrong destroyer, double destroy,
use-after-destroy, missing destroy, and borrowed destroy tests pass. ASan/UBSan
must be clean on the mandatory host lane.

## 12.5 Synchronous callbacks

- [ ] Canonical shape is function pointer plus opaque context and optional
  context destroyer.
- [ ] V4 callbacks are synchronous, non-retained, same-thread, non-concurrent,
  and non-reentrant unless a fixture explicitly proves a narrower safe case.
- [ ] Generated trampolines validate context and contain panic.
- [ ] The callback cannot be invoked after the foreign call returns or after
  context destruction.

Gate: success, foreign error, callback panic, attempted retention, double
destroy, reentry, and cross-thread negatives pass.

Primary examples:

- `examples/v4_c_record/`
- `examples/v4_c_entry_error/`
- `examples/v4_c_string_view/`
- `examples/v4_c_owned_buffer/`
- `examples/v4_c_opaque_handle/`
- `examples/v4_c_callback/`
- matching `examples/fail_v4_*` packages

Verification after every sub-slice:

- `make test`
- `make test-v4-c`
- `make abi-check`
- required sanitizer target for pointer/resource slices
- `make tree-test` and LSP inventory tests when the source/build surface changes

**STOP:** no sub-slice ships with an undocumented validity invariant, ownership
transition, lifetime, thread rule, panic path, or layout assumption.


# 13. M8 — Bounded C Header Import

Goal: turn real headers into the same verified import model without making the
compiler a C preprocessor/parser.

Primary files:

- `lang/tooling/fol-interop/` as the sole PARC/LINC/GERC orchestrator
- a thin CLI route under `lang/tooling/fol-frontend/src/cli/`
- `lang/compiler/fol-package/src/`
- `lang/compiler/fol-lower/src/abi.rs`
- `lang/execution/fol-build/src/` build-record semantic registry
- `flake.nix`
- header fixtures under `test/ffi/headers/`

Tasks:

- [ ] Consume the exact PARC revision and its normal `scan_headers` production
  API; FOL must not invoke libclang, parse a Clang AST, or add another header
  frontend.
- [ ] Construct one explicit PARC `TargetSpec` with the locked compiler
  identity, supported C standard, sysroot, include roots, defines, environment
  policy, and bounded preprocessing policy.
- [ ] Add `fol tool bind c` with human/plain/JSON output and deterministic
  target-specific manifest generation over the typed sibling pipeline.
- [ ] Add the explicit adapter annotation format for ownership, pointer/length,
  nullability, effect, escape, and destructor pairs.
- [ ] Canonicalize include roots and reject traversal/symlink escape; record
  header, annotation, toolchain, target, and relevant sysroot identities in the
  build fingerprint.
- [ ] Translate only the supported subset from Section 4.12.
- [ ] Emit structured diagnostics with header source ranges and exact unsupported
  construct names.
- [ ] Detect stale generated interfaces when headers/annotations/toolchain/
  target change.
- [ ] Make build-record completion offer only fields/values owned by the shared
  semantic registry.
- [ ] Keep unsupported functions absent/unusable rather than approximating
  their types.

Tests:

- deterministic import of scalar, POD record, opaque handle, and annotated
  slice headers
- same header for two targets yields distinct target manifests where required
- unsupported macro API, varargs, bitfield, packed, flexible array, union, C++,
  unknown calling convention, and unsafe pointer contract each reject clearly
- changed header/annotation invalidates stale output
- include path traversal/symlink escape rejects
- generated import is consumed by the M6 real FOL caller

Verification:

- `make test`
- `make test-v4-c`
- `make test-interop`
- `make interop-locked`
- `make abi-check`
- `make docs TYPE=mdbook`

**STOP:** header import cannot be called complete if it depends on the host
target implicitly, accepts unsupported declarations approximately, or lacks
explicit ownership/effect annotations for pointer/resource APIs.


# 14. M9 — Rust Source Facade and Rust Adapter Interop

Goal: prove both Rust directions while keeping the compatibility promise honest.

Primary files:

- `lang/execution/fol-backend/src/abi/`
- `lang/execution/fol-backend/src/emit/{skeleton.rs,build.rs}`
- `lang/tooling/fol-frontend/src/{compile,bind,cli}/`
- `examples/v4_rust_consumer/`
- `examples/v4_rust_provider/`
- `test/integration_tests/integration_v4_rust.rs`

FOL -> Rust tasks:

- [ ] Emit a Cargo-compatible source crate with `src/lib.rs`, stable package
  name, explicit edition, and exact runtime dependency metadata.
- [ ] Generate public safe Rust types/functions from the canonical interface;
  keep internal ID-mangled names and unsafe glue private.
- [ ] Express recoverables as safe Rust `Result` in the facade while preserving
  the same underlying ABI/error meaning.
- [ ] Express borrowed/owned values with Rust lifetimes/owned wrappers that call
  the manifest-paired release routine exactly once.
- [ ] Compile with `deny(improper_ctypes_definitions)` and
  `deny(unsafe_op_in_unsafe_fn)`.
- [ ] Record supported rustc range/toolchain metadata and diagnose mismatch.

Rust -> FOL tasks:

- [ ] Scaffold an adapter crate from a selected manifest/API contract.
- [ ] Make the adapter crate's own checked-in `Cargo.lock` and Cargo invocation
  own external dependencies/features/build scripts/proc macros.
- [ ] Produce a target-specific C ABI library + manifest that M6 consumes.
- [ ] Add one fixture with an enabled Cargo feature and a transitive dependency
  to prove the boundary is real.
- [ ] Run adapter builds locked/frozen/offline where the fixture permits.
- [ ] Never accept an arbitrary prebuilt `.rlib` as a durable compatible input.

Tests:

- separate Rust crate consumes installed FOL source facade and exercises scalar,
  record, error, view, and owned handle paths
- FOL calls Rust provider adapter with feature + transitive dependency
- toolchain/target/runtime metadata mismatch reports a structured diagnostic
- drop/release happens once across success, error, and panic
- public Rust API contains no lowered IDs or internal module names
- docs and CLI label the output "Rust source facade," not "stable Rust ABI"

Verification:

- `make test`
- new `make test-v4-rust`
- `make test-v4-c`
- `make abi-check`
- `make docs TYPE=mdbook`

**STOP:** M9 fails if Cargo and direct rustc become two disagreeing production
drivers, if arbitrary build scripts execute under FOL without an explicit trust
boundary, or if any documentation implies cross-rustc binary compatibility.


# 15. M10 — Compatibility, Tooling, Platform, and Release Closure

Goal: close every cross-cutting surface and prove release artifacts, not only
developer-tree builds.

Tasks:

- [ ] Add `fol tool abi inspect` and `fol tool abi check` (or the single exact
  M0-frozen spelling) backed by the canonical manifest implementation.
- [ ] Compare checked-in ABI baselines; distinguish compatible additions,
  breaking changes, target mismatch, and build-only fingerprint changes.
- [ ] Inspect actual symbols with target-appropriate LLVM/platform tools and
  compare against the allowlist.
- [ ] Verify SONAME/install-name/import-library/runtime lookup behavior without
  injecting a hidden default rpath.
- [ ] Run two clean builds and compare manifest/header/export lists and all
  declared reproducible outputs.
- [ ] Test concurrent builds and cache isolation.
- [ ] Package release archives with headers, libraries, import libraries,
  manifests, link interface, Rust facade, licenses, checksums, provenance, and
  SBOM.
- [ ] Extract each release archive in a clean directory and compile/link/run C
  and Rust consumers without repository-relative paths.
- [ ] Make the certified `x86_64-unknown-linux-gnu` lane release-blocking and
  keep candidate/experimental compile lanes explicitly non-certifying.
- [ ] Pin GitHub Actions and Rust/mdBook/tree-sitter/Clang/LLVM/C toolchain
  inputs rather than using mutable `latest` references.
- [ ] Make CI invoke Makefile-owned validation targets instead of duplicating a
  weaker command set.
- [ ] Update README, architecture, docs, book, examples, and version plan to
  present exactly the shipped matrix and remaining exclusions.

Verification:

- all targets in Section 17
- clean archive consumer tests on the certified platform
- no unexpected export on any certified shared library
- no checked-in generated-file drift after verification

**STOP:** V4 cannot close with a skipped certified platform, a release archive that
only works inside the repo, mutable toolchain/action inputs, or documentation
whose support matrix is broader than CI evidence.


# 16. Cross-Cutting Tooling and Editor Work

This is not a late phase. Apply the relevant rows in the same commit as every
M0-M10 slice, then perform the full audit in M10.

## Diagnostics

- stable ABI/link/backend codes with a recognized family
- exact primary source/header/build location
- related sites for conflicting export, bad nested field, provider, target, or
  destructor
- structured note/help/suggestion and `fol code explain`
- human/plain/JSON parity
- external command stdout/stderr as secondary context, not the primary message

Primary file: `lang/compiler/fol-diagnostics/src/explain.rs`, plus each
producer's real construction sites.

## Formatter and tool commands

- formatter preserves exact external names, header paths, annotation records,
  raw pointer qualifiers, and ABI export/build records
- formatting remains idempotent and compiler-analyzable
- `fol tool parse`, `highlight`, and `symbols` execute the generated tree-sitter
  assets for every new syntax/build shape
- `fol tool bind c` and ABI inspect/check share normal output modes and stable
  diagnostics

## LSP

- hover shows C/Rust name, calling convention, target, ABI shape, ownership,
  nullability, effect/tier, and destructor pair
- definition/navigation reaches original FOL declaration, imported header range,
  or generated manifest origin as appropriate
- references distinguish internal FOL references from foreign export/import
  edges
- rename never silently changes an external symbol; external rename is rejected
  or requires an explicit ABI-breaking build-record edit
- document/workspace symbols include foreign imports/exports with distinct kinds
- semantic tokens cover foreign declarations/raw address tokens without adding
  editor-owned legality
- completion covers M0-frozen build records, target values, calling conventions,
  effect/ownership enums, provider roles, and generated foreign namespaces
- diagnostics include related multi-file/header/build sites and correct UTF-16
  positions
- code actions remain narrow/exact; do not claim broad ABI rewrites

Primary files:

- `lang/tooling/fol-editor/src/lsp/{analysis.rs,semantic.rs,completion_helpers.rs}`
- `lang/tooling/fol-editor/src/lsp/tests/`
- `lang/execution/fol-build/src/semantic.rs` (the compiler-owned build semantic
  registry re-exported through `fol-package`)

## Tree-sitter

- grammar matches the one chosen raw-pointer/export/import/build spelling
- highlights distinguish ABI/build keys, operators, type names, and external
  names without hardcoding semantic availability
- locals/symbol queries understand any new declaration node; if imports are
  manifest-synthesized and add no source node, explicitly test that no grammar
  change is needed
- every positive V4 `.fol`/`build.fol` parses with zero ERROR nodes
- deleted `cast` and `.de_alloc` surfaces do not remain silently accepted as
  shipped V4 features
- generated grammar/query assets and external corpus stay synchronized

Primary files:

- `lang/tooling/fol-editor/tree-sitter/grammar.js`
- `lang/tooling/fol-editor/queries/fol/{highlights,locals,symbols}.scm`
- `lang/tooling/fol-editor/src/tree_sitter.rs`
- `lang/tooling/fol-editor/tree-sitter/test/corpus/v4_*.txt`

Rule: an editor change may be "none" only when a test proves the compiler-backed
path already supplies the correct behavior.


# 17. Test, Make, CI, and Platform Matrix

## 17.1 Canonical inventories

Add one compiler/test-owned inventory in `test/v4_example_inventory.rs`, reused
by integration tests and `fol-editor` tests. Use names that cannot be confused
with V3 processor milestone suffixes:

- positive: `v4_c_*`, `v4_rust_*`, `v4_build_*`
- negative: `fail_v4_*`

Inventory rows carry expected artifact/model/target, diagnostic code, message
fragment, related-site expectation, LSP expectation, tree-sitter corpus probe,
and whether a native toolchain is required.

Required positive groups:

- artifact/target/link foundation
- scalar C export and import
- POD record and entry/error
- string/slice view
- owned buffer and opaque handle lifecycle
- synchronous callback
- header import
- Rust consumer and Rust provider adapter

Required failure groups include every STOP condition and classifier rejection
in this plan.

## 17.2 Makefile targets

Keep `make` as the public validation interface. Add:

- `make fmt-check` — non-mutating `cargo fmt --all -- --check`
- `make lint` — workspace clippy with warnings denied
- `make test-native` — required host native artifact/link materialization
- `make test-v4-c` — C import/export consumer suite
- `make test-v4-bindgen` — pinned header-import suite
- `make test-v4-rust` — Rust source facade/adapter suite
- `make test-v4-sanitize` — host ASan/UBSan boundary/lifecycle suite
- `make test-v4-cross` — optional candidate-target promotion evidence; it does
  not certify or weaken the required native GNU/Linux lane
- `make abi-check` — manifests, baselines, symbols, layouts, fingerprints
- `make verify` — fmt-check, lint, build, all required tests including ignored
  tests, native/C/Rust/ABI, tree-test, generated cleanliness, and mdBook

`make test` must continue to run the full Rust workspace plus ignored tests and
must include non-optional host V4 integration tests. A missing optional
cross/sanitizer tool may skip only its explicitly optional lane with a clear
reason; it cannot turn required host V4 tests green.

## 17.3 Platform support tiers

Certified initial (release-blocking parse, inspect, probe, generate, compile,
link, and run):

- `x86_64-unknown-linux-gnu` with ELF and an explicit GCC toolchain identity

Candidate, non-blocking promotion lanes:

- `x86_64-unknown-linux-musl` after explicit sysroot/static-link evidence
- a second Linux architecture after native or policy-controlled emulated runs

Experimental and not advertised for V4 until sibling-native certification:

- `aarch64-apple-darwin` and `x86_64-apple-darwin`
- `x86_64-pc-windows-msvc` and MinGW targets

Do not infer support from rustc accepting a triple or from string-rendering
tests. Promotion is an explicit sibling-contract, native-fixture, CI, plan, and
documentation change. Apple frameworks, Mach-O install names, PE/import
libraries, and Windows ABI rules remain rejected outside such a promotion.

## 17.4 Boundary safety lanes

- generated Rust lints for FFI safety/unsafe bodies
- C `_Static_assert` and Rust size/align/offset/tag probes
- ASan/UBSan for C consumers and resource lifecycle
- Miri for generated Rust facade/adapter units where supported
- fuzz/property tests for manifest decoding and pointer/length/tag/UTF-8
  validators
- timeout/deadlock protection for callback tests
- exact symbol/export inspection
- archive-extraction consumer tests


# 18. Documentation and Book Matrix

Update these in the milestone that changes their truth:

- `README.md`
- `ARCHITECTURE.md`
- `docs/runtime-models.md` (foreign effects do not create a model)
- `docs/editor-sync.md`
- `book/src/050_tooling/100_frontend.md`
- `book/src/050_tooling/200_tool_commands.md`
- `book/src/050_tooling/300_editor.md`
- `book/src/050_tooling/350_compiler_integration.md`
- `book/src/050_tooling/400_treesitter.md`
- `book/src/050_tooling/450_feature_checklist.md`
- `book/src/050_tooling/500_lsp.md`
- `book/src/055_build/{100_build_file,200_graph_api,300_handle_api,400_options,600_artifacts,700_cross_compilation,900_direction}.md`
- `book/src/400_type/100_ordinal.md`
- `book/src/500_items/200_routines/_index.md`
- `book/src/600_modules/100_import.md`
- `book/src/650_errors/300_diagnostics.md`
- `book/src/700_sugar/250_dfr.md`
- `book/src/750_conversion/{100_coercion,200_casting}.md`
- `book/src/800_memory/200_pointers.md`

Add a dedicated book section and link it in `book/src/SUMMARY.md`:

- `book/src/950_interop/_index.md`
- `book/src/950_interop/100_c_abi.md`
- `book/src/950_interop/200_c_import.md`
- `book/src/950_interop/300_rust.md`
- `book/src/950_interop/400_abi_compatibility.md`

Documentation must include:

- exact shipped type/support matrix
- exact capability/effect rules
- C header/manifest/install examples
- target and link diagnostics
- ownership/destroy/status/panic/callback examples
- header importer supported/unsupported matrix
- Rust source compatibility statement
- ABI versioning/baseline workflow
- platform tiers and runner limitation
- explicit non-goals

No future module/type/provider is documented as available before a checked-in
example and real consumer test exist.


# 19. Security, Reproducibility, and Supply-Chain Checklist

- [ ] Canonicalize all native/header/sysroot/tool paths and reject traversal or
  symlink escape from declared roots.
- [ ] Validate native filenames/library names and never interpolate them into a
  shell command.
- [ ] Use structured `Command` arguments; no shell-expanded linker fragments.
- [ ] Inspect object format/architecture before linking.
- [ ] Cryptographically digest exact native binary/header/annotation inputs and
  record provider provenance in lock/build metadata.
- [ ] Do not use `DefaultHasher` as a native-content identity.
- [ ] Hash/redact environment values in reports/cache identity.
- [ ] Lock per-plan temporary outputs and publish atomically.
- [ ] Validate every C-originating pointer, length, alignment, capacity, tag,
  bool, Unicode, UTF-8, output pointer, and ownership token before use.
- [ ] Keep generated unsafe code minimal, linted, locally documented, and
  inaccessible from safe facade APIs.
- [ ] Catch/translate panic; reject unwind-capable foreign declarations.
- [ ] Pair every owned value with one exact provider/domain destroy path.
- [ ] Prevent callback retention, post-destroy invocation, unapproved reentry,
  and cross-thread invocation.
- [ ] Treat header importer input and Rust adapter build scripts/proc macros as
  explicit trusted build inputs; FOL does not execute arbitrary dependency code
  implicitly.
- [ ] Pin CI actions and toolchain versions; publish checksums, provenance, and
  SBOM with release artifacts.


# 20. Risk Register

| Risk | Consequence | Prevention / early signal |
|---|---|---|
| build graph remains metadata-only | headers/libraries bypass caching/install and drift | M2 materializer gate before ABI outputs |
| target aliases diverge | host artifact mislabeled or wrong native input linked | one fallible `ResolvedTarget`; no process launch on unknown target |
| current Rust layout leaks | permanent unstable/UB-prone ABI | canonical facade shapes; classifier/verifier; symbol/layout probes |
| scalar/order information remains erased | wrong header widths/offsets/tags | M4 preservation tests before backend wrappers |
| public names use internal IDs | harmless refactor breaks ABI | explicit allowlist names; reorder/determinism tests |
| Cargo scope expands silently | two build truths, network/build-script trust problems | source/adapter boundary; broad ingestion explicitly out |
| raw pointers become general unsafe memory | UAF, aliasing, allocator mismatch | address-token-only rule; no deref/arithmetic/casts; paired destroy |
| panic or invalid C values enter Rust unchecked | UB or foreign unwind | uniform status/out wrappers and validators; sanitizer/fuzz lanes |
| platform matrix explodes | permanent partial support and vague claims | one certified lane plus explicit candidate/experimental promotion tiers; unsupported triples reject honestly |
| docs lead code | false completion claim | truth repair in M0; real consumer gate before shipped wording |
| editor mirrors lag | compiler-only feature and broken UX | cross-cutting inventory reused by LSP/tree-sitter/tool tests |
| ABI hash includes build noise | every compiler update looks breaking | separate interface/build fingerprints |
| ABI hash omits safety metadata | incompatible ownership/error change passes | manifest fingerprint includes all public semantic contracts |
| parallel builds share/delete outputs | flaky/corrupt artifacts | per-plan lock/temp/atomic publication tests |
| foreign resource cleanup is hidden | leaks/double-free/wrong allocator | explicit ownership states, destroy pairs, lifecycle negatives |


# 21. Recommended Implementation and Commit Order

Land in this order only:

1. M0 — contract fixtures, characterization, truth repair
2. M1 — target + resolved artifact plan
3. M2 — action graph/materializer
4. M3 — backend artifact families + native link plan
5. M4 — scalar/order preservation + canonical foreign model
6. M5 — scalar C export
7. M6 — scalar C import/provider resolution
8. M7.1 — POD records
9. M7.2 — entries/recoverable errors
10. M7.3 — borrowed views
11. M7.4 — owned buffers/opaque handles
12. M7.5 — synchronous callbacks
13. M8 — bounded header import
14. M9 — Rust source facade/provider adapter
15. M10 — compatibility/platform/release closure

Docs, diagnostics, frontend, formatter/tools, LSP, tree-sitter, examples, and
inventories travel with each numbered slice, not after step 15.

Commit policy during implementation:

- commit after each coherent green slice; do not accumulate multiple milestones
  in one unreviewable commit
- run the slice's Make targets before committing
- use unsigned, title-only Conventional Commit messages, maximum 50 characters
- never add a signature, body, co-author footer, or generated attribution
- no `wip` commits on the feature branch

Representative titles (adjust scope without exceeding the limit):

- `docs(v4): freeze interop contract`
- `fix(build): preserve artifact plans`
- `feat(build): materialize graph actions`
- `feat(backend): emit native libraries`
- `feat(abi): preserve scalar layouts`
- `feat(abi): add foreign interface`
- `feat(abi): ship C exports`
- `feat(abi): ship C imports`
- `feat(abi): add owned handles`
- `feat(interop): add Rust facade`
- `test(v4): add platform ABI gates`
- `docs(v4): close interop milestone`

Do not expose a half-feature merely to make a commit smaller. Structural work
may land privately; the public surface appears only with its complete vertical
consumer/editor/docs slice.


# 22. Hard STOP Conditions

Stop the active milestone immediately if any of the following is true:

- scalar width/sign/encoding is still erased
- record order or entry discriminants depend on a map/traversal/internal ID
- an unknown target can omit `--target` and build for the host
- an artifact field is reconstructed from defaults downstream
- library compilation still requires `main`
- object output cannot describe its remaining link requirements
- native input target/provenance/digest is unknown
- local artifact linking duplicates compiled source
- link cycles/order are delegated to accidental linker behavior
- headers and wrappers consume separate semantic models
- a public symbol contains internal type/routine/package traversal IDs
- `[exp]` alone implies native export
- a runtime `String`, `Vec`, `Option`, `Result`/`FolRecover`, Rust `bool`/`char`,
  `Rc`, channel, eventual, mutex, or default-layout enum crosses directly
- a generic or anonymous structural type reaches ABI emission
- a raw pointer lacks constness, nullability, ownership, escape, or destroy
  provenance
- raw dereference, arithmetic, integer-pointer conversion, or general unsafe
  code slips into V4
- `.de_alloc` acts as a universal free
- a boundary input becomes a Rust reference/slice/value before validation
- a panic/unwind/exception/`longjmp` can cross C
- a foreign call bypasses `fol_model`/effective-std capability checks
- callbacks can be retained, reentered, or cross threads outside the frozen rule
- header import approximates an unsupported C construct
- Rust binary ABI is described as stable
- arbitrary Cargo crates/build scripts are ingested without a separate approved
  trust/dependency/toolchain contract
- Cargo and direct rustc become parallel production truths
- an interface fingerprint changes from an internal-only refactor
- a breaking interface change passes without ABI-major bump
- a required native/editor/tree/docs/platform test is skipped or passes
  vacuously
- docs claim a wider surface than real consumer/CI evidence


# 23. Explicit Non-Goals

Not part of V4 unless this plan is deliberately revised first:

- stable Rust binary ABI or arbitrary prebuilt `.rlib` compatibility
- a Rust `dylib` compatibility promise
- C++ ABI, templates, overloads, exceptions, or name mangling
- C varargs
- unrestricted C macro import
- bitfields, packed structs, flexible arrays, arbitrary unions, SIMD/vector ABI
- arbitrary pointer arithmetic/dereference or a general unsafe language mode
- universal manual free or allocator-agnostic deallocation
- general transmute/bitcast/pointer cast/container cast
- lossy numeric casts without a separate checked-result design
- implicit generic ABI exports
- exported/imported globals or TLS
- standards/vtable/trait-object ABI
- closure ABI beyond the bounded synchronous callback descriptor
- retained, concurrent, cross-thread, or asynchronous callbacks
- channels, eventuals, mutexes, tasks, async, or generators across the ABI
- cross-language exceptions/unwind/`longjmp`
- automatic `pkg-config`, vcpkg, CMake, or arbitrary provider discovery
- raw linker flags, scripts, or response files in `build.fol`
- automatic bindings for every C-compatible language
- cross-target execution without an explicit runner
- weak references/cycle collection merely because V4 touches native resources


# 24. Completion Rule

V4 is complete only after M0-M10 and every required M7 sub-slice ship through
compiler truth, runtime/backend, operational build/link/install routing,
structured diagnostics, frontend artifacts, formatter/tool commands, LSP,
tree-sitter grammar/queries/corpus, canonical examples/failures, platform CI,
release archive consumers, docs, and book.

The final claim must be precise:

> FOL V4 provides a versioned target-specific C ABI with real C import/export,
> bounded header import, explicit ownership/status/panic rules, target-aware
> native artifacts and linking, and source-level Rust facades/adapters. It does
> not promise stable Rust binary ABI, general unsafe pointers, C++, arbitrary
> Cargo ingestion, or async/runtime-object interop.

If that sentence is not fully backed by the checked-in consumer matrix and ABI
baselines, V4 remains partial.
