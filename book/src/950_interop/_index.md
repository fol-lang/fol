# Interop Toolchain Boundary

The repository-wide hardening gate is complete for the initial certified
`x86_64-unknown-linux-gnu` lane. The locked H7 Make gate passes in FOL CI
through clean, exact PARC, LINC, and GERC commits. This unblocks the first
broader V4 milestone; it does not complete FOL V4's broader language-level
interop milestones.

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
| PARC | `follang-parc 0.16.0` | source package schema 2 | `0f52aeeeeec47a082c0d8a515130ee853aa1101d` |
| LINC | `follang-linc 0.1.0` with `native-inspection` | link-analysis schema 2 | `fdb50ae9743ee09c6592bc40e87b6f57a892fc51` |
| GERC | `follang-gerc 0.1.0` with `pipeline-native` | generation domain 1 | `fdf139209e824dd7fdc274da20b6be3ba0183a10` |

The lock also freezes the GERC H5 compatibility driver, fixtures, and support
code under digest
`13644fd1f6ad3f1de06338e5bd415604dbedc9b6baaaaed8a63f44515db004e7`.
The three components are **git dependencies pinned by revision**, not sibling
path dependencies, so a fresh clone of this repository builds and tests without
any of them checked out. A pinned revision is content-binding, which makes
provenance a build-time property rather than something to re-observe at run
time:

- `fol-interop/build.rs` proves that every revision and remote in
  `interop.lock.toml` equals what cargo resolved in `Cargo.lock`, and hands the
  verified revisions to the crate. A stale lockfile is a compile error.
- compile-time assertions in `fol-interop/src/lib.rs` prove the resolved crates
  still expose the contract versions the lock claims.
- each component pins the components below it at the same revisions this lock
  records (`parc_revision`, `linc_revision`), because two different `follang-parc`
  revisions in one graph would produce two incompatible sets of contract types.

The earlier design shelled out to `git` at run time to check each sibling
checkout's root, `HEAD`, worktree cleanliness, and origin. That could only pass
inside a source tree — a released binary carried the build machine's paths — so
it verified nothing for users.

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

- `make interop-check` runs the offline tier: lock self-consistency, the
  cross-pins between components, agreement with `fol-interop/Cargo.toml` and
  `Cargo.lock`, the absence of any source override, and compilation of the typed
  integration. It needs no network and no resolved checkouts.
- `make interop-locked` adds the resolved tier: it fetches with `--locked`,
  finds each component's checkout in the cargo git cache, and verifies its
  revision, manifest identity, schema constants, features, and the H5 corpus
  digest.
- `make test-interop` depends on the locked check, requires Linux and GCC, and
  runs the positive and fail-closed native H7 tests without an optional-skip
  path.

CI checks out only FOL and invokes the same Make-owned locked smoke gate; cargo
fetches the pinned components. Moving a component means updating
`interop.lock.toml`, `fol-interop/Cargo.toml`, `Cargo.lock`, and this snapshot
together — the offline tier fails the moment any one of them disagrees.

Working on a component locally without editing the pins:

```toml
# ~/.cargo/config.toml
[patch."https://github.com/fol-lang/parc"]
follang-parc = { path = "/absolute/path/to/parc" }
```

With repository-wide hardening complete, this boundary unblocks the first
broader V4 work. It does not itself expose general foreign declaration
syntax, general pointers or ownership, C export, bounded header-import
tooling, C++ ABI support, Rust facade generation, or a stable Rust binary ABI.
