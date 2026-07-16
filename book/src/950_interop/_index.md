# Interop Toolchain Boundary

The locked H7 smoke handoff is implemented and its focused Make gate passes
for the initial certified lane. Final repository-wide hardening sign-off is a
separate gate. This is a narrow production smoke path through the sibling
toolchain, not completion of FOL V4's broader language-level interop
milestones.

FOL integrates three independently usable sibling crates and does not copy
their native semantics:

```text
build.fol executable + one C header/object import
  -> PARC CompleteSourcePackage
  -> LINC ValidatedLinkAnalysis
  -> GERC GenerationBundle
  -> fol-build generated-file action graph
  -> fol-backend auxiliary Rust crates and exact rustc arguments
  -> linked and executed FOL binary
```

The stages have fixed ownership:

- PARC is the only C preprocessor, parser, recovery engine, source extractor,
  provenance store, and source-contract owner.
- LINC is the only native artifact inspector, compiler/ABI probe runner,
  symbol/provider validator, and ordered link-evidence owner.
- GERC is the only closed-world raw Rust FFI projector and emitter.
- FOL owns language policy, target and build-graph routing, generated-file
  materialization, the narrow H7 call anchor, backend process invocation,
  diagnostics, and eventual safe language wrappers.

The handoff is typed. FOL does not use JSON shape conversion, copied sibling
models, a second provider resolver, a second raw `extern "C"` emitter, shell
splitting, or text link-argument parsing. GERC's typed link atoms remain
individual native process arguments when they reach `rustc`.

## Locked inputs

The checked root `interop.lock.toml` is the machine authority. H7 is certified
against this exact snapshot:

| Stage | Package | Contract | Locked revision |
|---|---|---|---|
| PARC | `follang-parc 0.16.0` | source package schema 2 | `ba603cdccc9375473eca0c42e5462cf90b6da249` |
| LINC | `follang-linc 0.1.0` with `native-inspection` | link-analysis schema 2 | `37c8fb16171114b39e2283ff4b9e351fa2d5975b` |
| GERC | `follang-gerc 0.1.0` with `pipeline-native` | generation domain 1 | `423b14aec40f509de64152ec1fcc74a9371154f1` |

The lock also freezes the GERC H5 compatibility driver, fixtures, and support
code under digest
`1feaeb4f9f0aa2275a3973217e866d8fd9942d4266f2b97c28bc6215403757f5`.
`Cargo.lock` alone cannot record Git identities for path dependencies.
The production H7 route therefore observes each compiled sibling path's
canonical Git root, `HEAD`, worktree status, and normalized origin before any
sibling API runs. Revision values enter the evidence report only after those
runtime checks match the compiled lock identities.

## Certified platform

The only promoted lane is:

```text
x86_64-unknown-linux-gnu
ELF, LP64
explicit GCC executable and observed compiler identity
one executable artifact with one C object provider
```

The caller supplies normalized absolute paths for GCC and the bounded LINC
probe workspace. LINC observes and fingerprints the compiler rather than FOL
guessing its identity. The selected FOL target must equal every sibling target
fingerprint before generated files or backend compilation are allowed.

Linux musl, other Linux architectures, Apple targets, Windows targets,
frameworks, import libraries, multiple imports, and the general C type/API
surface are not certified by H7. Clang remains sibling differential evidence;
it is not the compiler for FOL's promoted H7 lane.

## Evidence and failure policy

The required smoke test starts from the real `build.fol` graph route. It
compiles a C provider object, scans its header through PARC, certifies the
provider through LINC, projects raw Rust through GERC, materializes the raw and
FOL-owned anchor crates through `fol-build`, passes exact ordered link
arguments to `fol-backend`, and runs the linked executable. Its reported
evidence contains:

- the three locked sibling revisions;
- source, target, link-analysis, generation, and provider fingerprints;
- the exact certified target.

The checked build separately retains the exact generated raw-binding and
anchor crate roots passed to the backend. The system test inspects the fixed
anchor source, builds both crates, executes the final binary, and verifies the
provider's per-run return value.

Required negative cases prove that partial PARC source, unresolved LINC
providers, and GERC generation rejection stop before generated/backend files
are written. Target mismatch is rejected before compiler or output-directory
I/O. A skipped system test is not success: the required Make target sets
`FOL_H7_REQUIRED=1` and supplies an explicit canonical GCC path.

## Verification commands

Run these on GNU/Linux from the FOL root with `parc`, `linc`, and `gerc` as
sibling checkouts:

```sh
make interop-check interop-locked test-interop
```

- `make interop-check` checks lock shape, package/schema/feature compatibility,
  the H5 corpus identity, and compilation of the typed integration. It is the
  development check and does not require sibling `HEAD`s to equal the lock.
- `make interop-locked` additionally requires each sibling to have the exact
  locked `HEAD`, canonical GitHub origin, and a clean worktree.
- `make test-interop` depends on the locked check, requires Linux and GCC, and
  runs the positive and fail-closed native H7 tests without an optional-skip
  path.

CI checks out FOL plus all three repositories in that sibling layout, installs
Rust 1.89.0, and invokes the same Make-owned locked smoke gate. Changing a
sibling revision requires changing `interop.lock.toml`, compiled lock
constants, CI checkout refs, compatibility evidence, and this snapshot
together.

After final repository-wide hardening sign-off, this boundary unblocks the
first broader V4 work. It does not itself expose general foreign declaration
syntax, general pointers or ownership, C export, bounded header-import
tooling, C++ ABI support, Rust facade generation, or a stable Rust binary ABI.
