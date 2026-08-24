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

## Pinned inputs

`lang/tooling/fol-interop/Cargo.toml` is the machine authority: each component
is pinned there by git revision, and `Cargo.lock` records what cargo resolved
from those pins. H7 is certified against this exact snapshot:

| Stage | Package | Contract | Pinned revision |
|---|---|---|---|
| PARC | `follang-parc 0.16.0` | source package schema 2 | `0f52aeeeeec47a082c0d8a515130ee853aa1101d` |
| LINC | `follang-linc 0.1.0` with `native-inspection` | link-analysis schema 2 | `38f73db2d14e3b877e430d59220c8ea0d6c92e85` |
| GERC | `follang-gerc 0.1.0` with `pipeline-native` | generation domain 1 | `df0479a1074a1a3d50022ca6a28cdec4221987dd` |

The three components are **git dependencies pinned by revision**, not sibling
path dependencies, so a fresh clone of this repository builds and tests without
any of them checked out. A pinned revision is content-binding, which makes
provenance a build-time property rather than something to re-observe at run
time:

- `fol-interop/build.rs` reads the resolved revisions back out of `Cargo.lock`
  and hands them to the crate, so the recorded provenance is whatever cargo
  actually built against.
- compile-time assertions in `fol-interop/src/lib.rs` prove those crates still
  expose the expected contract versions.
- each component pins the components below it at these same revisions, because
  two different `follang-parc` revisions in one graph would produce two
  incompatible sets of contract types — cargo resolves them as separate crates
  and every shared type stops matching.

`tools/verify-interop-lock.sh` checks that nothing loosens a pin to a branch or
a path, that no `[patch]` entry substitutes a component, and that this table
still quotes the revisions in force.

The earlier design shelled out to `git` at run time to check each sibling
checkout's root, `HEAD`, worktree cleanliness, and origin. That could only pass
inside a source tree — a released binary carried the build machine's paths — so
it verified nothing for users.
## Certified platforms

Two lanes are promoted:

```text
x86_64-unknown-linux-gnu     ELF, LP64
x86_64-unknown-linux-musl    ELF, LP64
explicit GCC or clang executable and observed compiler identity
one executable artifact with one C object provider
```

glibc and musl are the same System V AMD64 ABI — only the libc differs — and
LINC measures every layout by compiling probes with the caller's own compiler
rather than modelling it, so the same evidence certifies both. Each lane has
its own link-and-run smoke; neither stands in for the other.

The caller supplies normalized absolute paths for the compiler and the bounded
LINC probe workspace. LINC observes and fingerprints the compiler rather than
FOL guessing its identity, and accepts the GCC and clang families. The selected
FOL target must equal every sibling target fingerprint before generated files
or backend compilation are allowed.

Other Linux architectures, Apple targets, Windows targets, frameworks, import
libraries, multiple imports, and the general C type/API surface remain
uncertified.
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

## The annotation overlay

C cannot say which of its declarations FOL may call, what a return code means,
or who releases a pointer. The overlay is where a header author states those
facts explicitly; the compiler never guesses them. It is a strict subset of
TOML, and every key it does not recognize is refused rather than ignored.

```toml
version = 1

[routine.c_math_add_one]
error = "infallible"

[routine.c_math_checked_div]
error = "status"
status_ok = [0]
status_error = [1, 2]
out = "result"
effects = ["allocates"]
```

`error` admits exactly two conventions. `errno`, a last-error slot, an
undocumented sentinel, `unwind`, and `longjmp` are each refused *by name*, with
the reason, rather than falling through to "unknown value": a convention FOL
cannot check is one FOL will get wrong.

### Handle domains

A pointer to an incomplete type is C's opaque handle, and the pointer itself
carries no ownership information at all. A domain declares one:

```toml
[handle.Widget]
destroy = "widget_free"

[routine.widget_new]
error = "infallible"
handle = "Widget"
handle_role = "produces"

[routine.widget_size]
error = "infallible"
handle = "Widget"
handle_role = "borrows"

[routine.widget_free]
error = "infallible"
handle = "Widget"
handle_role = "consumes"
```

The role is per routine, not per type, because the same `Widget *` is produced
by one call, lent to many, and released by one. A producer's handle is its
result; a borrower's or consumer's is its single pointer parameter, and a
routine with more than one pointer parameter is refused rather than having its
handle chosen by position.

The **domain is the identity**. It becomes a distinct ABI type, so a handle
from one provider can never reach another provider's destroy — that is a type
error rather than a runtime hazard. Four ways of making the identity incoherent
are refused: a domain whose destroy is not a selected routine, a destroy that
does not declare itself the consumer, a second consumer of one domain, and a
routine naming a domain no `[handle.<Name>]` table declares.

Which routine owes the release is part of the **interface** fingerprint, so
changing `borrows` to `consumes` invalidates every caller.

On the FOL side a handle is a [linear resource](../800_memory/170_linear.md):
consumed exactly once, explicitly, on every path. The runtime half of that
binding — the generated destroy wrapper and the FOL-side handle value — is not
built yet, and a handle-typed signature is refused by name rather than rendered
as a guess.

## Verification commands

Run these on GNU/Linux from the FOL root with `parc`, `linc`, and `gerc` as
sibling checkouts:

```sh
make interop-check interop-locked test-interop
```

- `make interop-check` runs the offline tier: every component pinned to a
  40-character revision rather than a branch or a path, no `[patch]` entry
  substituting one, this page still quoting the revisions in force, and
  compilation of the typed integration. It needs no network.
- `make interop-locked` adds `--locked` resolution, proving the pins resolve
  exactly as committed with no network access and no manifest edit.
- `make test-interop` depends on the locked check, requires Linux and GCC, and
  runs the positive and fail-closed native H7 tests without an optional-skip
  path.

CI checks out only FOL and invokes the same Make-owned locked smoke gate; cargo
fetches the pinned components. Moving a component means updating
`fol-interop/Cargo.toml`, `Cargo.lock`, and this snapshot together — the check
fails the moment the book stops quoting the revisions in force.

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
