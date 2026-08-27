# Interop Toolchain Boundary

FOL exports a real C ABI and imports real C libraries. A C program links a FOL
static or shared library and calls it through a generated header; a FOL package
calls a C provider through a manifest that `fol tool bind c` wrote from that
provider's own header and archive. Both directions are proven by lanes in
`make verify` that compile and run the result, not by reading generated text.

What crosses each way is listed under [What crosses](#what-crosses), together
with what is refused. The exclusions are as load-bearing as the inclusions:
FOL rejects a construct it cannot model rather than approximating it.

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
| PARC | `follang-parc 0.16.0` | source package schema 2 | `fffe6a2f7191b3618231a7603664fe18eceba4c1` |
| LINC | `follang-linc 0.1.0` with `native-inspection` | link-analysis schema 2 | `891269daafcbb2c81182309e1be3ecd49a773543` |
| GERC | `follang-gerc 0.1.0` with `pipeline-native` | generation domain 1 | `6ece4d3a0a3312c5056e8434a18279c561c7e83a` |

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
libraries, and multiple imports per artifact remain uncertified.

## What crosses

The two directions are not symmetric, and the asymmetry is real rather than an
accident of what has been tested. Everything below is exercised by a lane in
`make verify` that compiles and runs it; nothing is listed on the strength of
the code appearing to support it.

Exporting, FOL to C — a C program links a FOL library and calls it:

| Shape | Crosses as |
|---|---|
| integers `int[8..64]`, unsigned, `flt[32]`/`flt[64]` | the exact-width C scalar |
| `bol` | `fol_bool_t` (`uint8_t`); only 0 and 1 are valid, and imports validate |
| `chr` | `fol_char_t` (`uint32_t`); validated as a Unicode scalar value |
| records | a C struct whose layout C agrees with, field order preserved |
| entries | a C enum with the discriminants the variants state |
| resources | an opaque `fol_<domain>_t *`, released by a paired destroy |
| a routine parameter | a C function pointer plus its own `void *` context |
| recoverable errors | `fol_status_t` plus a typed error out-parameter |
| a FOL panic | contained and reported as `FOL_STATUS_PANIC` |
| borrowed text, inbound | `fol_str_view_t`, caller-owned and call-scoped |

Importing, C to FOL — FOL calls a C provider through a checked manifest:

| Shape | Crosses as |
|---|---|
| scalars, including through `typedef` chains | the measured FOL scalar |
| named structs and enums, as parameters | a FOL record and a FOL scalar |
| pointers and out-parameters | a FOL value on the success channel |
| a pointer/length pair | one borrowed FOL vector, its length derived |
| a provider-allocated buffer | a FOL vector, copied and released in the call |
| status codes | FOL's ordinary or recoverable channel, per the overlay |
| opaque handles | a `lin` linear resource with a paired destroy routine |
| one synchronous callback shape | a FOL closure invoked by the provider |

Both directions now carry named aggregates, opaque handles, and the one
callback shape. What is still refused, rather than approximated:

- **Importing** a C struct as a *result*. A struct crosses as a parameter,
  where the adapter rebuilds it from its fields; there is nowhere to put the
  fields of one that comes back.
- **Importing** a self-referential struct's pointed-to type. The pointer
  crosses as an opaque handle; the shape behind it does not.
- Variadics, bitfields, packed and flexible-array members, unions, C++
  linkage, and unknown calling conventions.
- Adopting a provider's allocation rather than copying it: FOL's allocator did
  not make it and must not free it.
- Raw pointers in an ordinary export: a raw pointer with no declared ownership
  and destroy pairing is refused, not defaulted.
- An opaque handle **inside a callback's signature**. A declared domain is read
  for a routine's own parameters and result, but not for the parameters and
  result of a callback it takes, so a callback returning `Widget *` is refused
  as incomplete while the same `Widget *` binds as the routine's own result.
  A callback returning a scalar beside it binds.
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
consumed exactly once, explicitly, on every path. The domain becomes a FOL type
in the import's namespace, so a program writes `wid::Widget` and the compiler
proves the release. `examples/v4_c_opaque_handle` is the whole path, and the
four `examples/fail_v4_c_handle_*` packages are the misuses C would compile.

### Buffers

C carries a buffer as two parameters with nothing joining them:
`checksum(const uint8_t *bytes, size_t count)` could as easily be a pointer and
an unrelated tally. The overlay pairs them:

```toml
[routine.digest_sum]
error = "infallible"
buffer = "bytes"
buffer_length = "count"
reads = ["bytes"]
```

`count` then stops being a FOL parameter. The length is **derived** from the
value FOL passes, so there is no second number a caller can get wrong and no
way to describe a buffer longer than the one that exists. On FOL's side it is a
borrowed vector -- `bor[vec[u8]]` when the provider only reads, a mutable loan
when it writes.

**Direction is declared, not read off constness.** `reads`, `writes`, and
`reads_writes` name the parameters they apply to. Constness stays the default,
but it is a poor witness: `void *base` in `qsort` is read and written, `char
*dst` in `strcpy` is only written, and a mutable pointer a provider never
writes looks like either. A declaration C contradicts -- `writes` on a const
pointee, or any direction on a by-value parameter -- is refused.

An **owned** buffer is memory the provider allocated, and it gets a domain and
a release, exactly like a handle:

```toml
[buffer.Bytes]
destroy = "digest_release"

[routine.digest_take]
error = "infallible"
buffer_domain = "Bytes"
buffer_role = "produces"
buffer_length = "out_len"
buffer_capacity = "out_capacity"

[routine.digest_release]
error = "infallible"
buffer_domain = "Bytes"
buffer_role = "consumes"
buffer_length = "count"
```

FOL never adopts that memory. Its allocator did not make the allocation and
must not free it, so the adapter **validates the report, copies out of it, and
calls the release** before returning. What FOL holds afterwards is its own
vector pointing nowhere near the provider's heap, and the destroy is not
mountable: there is nothing left for a program to release, and no address to
release it with.

Two ways a provider can contradict itself are refused rather than read: a
**null address with a nonzero length**, which describes memory that does not
exist, and a **length past the capacity** it reported, which describes memory
it did not allocate. Copying on either reads whatever happened to be there.
The capacity is what makes the second checkable -- a length on its own is
unfalsifiable.

The domain gets the same four cross-checks a handle domain does: a destroy that
is not a selected routine, a destroy that does not declare itself the consumer,
a domain with no producer or more than one, and a routine naming a domain no
`[buffer.<Name>]` table declares.

`examples/v4_c_buffer` runs all three shapes and asks the provider how many
allocations are outstanding afterwards -- the answer is 0, and FOL cannot see
C's heap any other way. `examples/fail_v4_c_buffer_capacity` and
`fail_v4_c_buffer_null` are the two contradictions.

### Entry discriminants

An entry crosses as a C enum, and a C enum's values are part of the ABI: they
are written to files and sent over wires. So FOL will not pick them. Each
variant states its own tag:

```fol
typ[exp] Lookup: ent = {
    con[tag = 4] MISSING;
    con[tag = 1] FOUND;
    con[tag = 9] DENIED;
};
```

`[tag = N]` is **not** a default value. `con NAME: int = 7` gives the variant a
default payload of 7, and `var Ok: int = 1` beside `var Err: str = "broken"`
shows why that cannot double as a tag — both variants would land on 1. The two
are different things and are now spelled differently.

An entry with no tags is refused outbound with `UnstableEntryTag`, naming the
entry. Its variants are numbered by position, which is fine inside FOL and
cannot be promised to anyone holding a stored value: inserting a variant would
renumber every later one.

Three ways of tagging incoherently are refused at parse time: a **duplicate**
tag, a tag outside the **32-bit range** the discriminant carries, and a
**partially tagged** entry, where the untagged variants would fall back to the
position the tags exist to escape.

Because a tag is stated rather than positional, **reordering the declaration is
a no-op**: the manifest is byte-identical, and `fol tool abi check` reports the
surface unchanged. Changing a tag is the opposite — a breaking change, refused
without `--allow-breaking`. `examples/v4_c_export_entry` crosses both
directions; `examples/fail_v4_c_entry_error` is the untagged refusal.

### Synchronous callbacks

A C routine that calls back needs a function pointer and somewhere to keep the
caller's state. C cannot say which `void *` belongs to which function pointer,
so the overlay does:

```toml
[routine.tally_range]
error = "infallible"
callback = "step"
callback_context = "context"
```

Neither position is a FOL parameter. FOL passes a closure and fills the context
itself, with that closure's address, so the pairing cannot be got wrong:

```fol
var total: int[32] = tal::tally_range(4, fun(a: int[32], v: int[32]): int[32] = {
    return a + v;
});
```

Bridging the two is a **generated trampoline** — a monomorphic `extern "C"` shim
nested inside the one call site, which recovers the closure from the context and
calls it. A Rust closure carries an environment and a C function pointer has
nowhere to put one; the trampoline is where that gap is closed.

**Exactly one shape is imported.** The function pointer's own first parameter
must be the `void *` context:

```c
int tally_range(int upto,
                int (*step)(void *context, int accumulator, int value),
                void *context);
```

A provider that puts its context last is refused at bind time. C permits that
form and real APIs use it, but FOL cannot tell a trailing context from any other
trailing pointer, and guessing wrong hands the provider an address that is not
the closure.

The trampoline enforces two rules rather than documenting them. A **null
context** means the callback was invoked outside the call that lent the closure.
A **fault inside the callback** cannot travel further: unwinding out of
`extern "C"` is undefined, and a callback has no status channel — the provider
is mid-call and takes only a return value. Both end the process, naming the
symbol, rather than returning a value nobody computed.

What FOL does **not** enforce is what the provider does after the call returns.
Retention, reentry, and cross-thread invocation are all provider behaviour that
nothing on FOL's side observes. FOL's half of the contract holds structurally —
the closure is a local, so the context cannot outlive the call — and the
null-context check catches the one detectable case. A real guarantee would need
a generation counter in the context and a provider-side destroyer, which V4 does
not build.

See `examples/v4_c_callback`, `examples/fail_v4_c_callback_panic`, and
`examples/fail_v4_c_callback_shape`.

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

This boundary does not expose general foreign declaration syntax, general
pointers or ownership, C++ ABI support, Rust facade generation, or a stable
Rust binary ABI. C export and bounded header import are built on top of it and
are described above.

## ABI versioning

A library that other people link needs to say when its surface changed, and to
distinguish that from having been rebuilt. An exporting artifact declares a
version beside its exports:

```fol
lib.set_abi_version({ major = 1, minor = 0 });
lib.add_abi_export({ routine = "add_i32", symbol = "fol_slice_add_i32" });
```

The installed manifest carries **two** fingerprints, and the split is the whole
point. The *interface* fingerprint covers what a caller can see: symbols,
types, layouts, error contracts, ownership. The *build* fingerprint covers how
the surface was produced — compiler, component revisions, link inputs. A new
compiler moves the build fingerprint and leaves the interface fingerprint
alone, so a toolchain upgrade does not read as an ABI event.

`fol tool abi check --baseline <MANIFEST> --candidate <MANIFEST>` compares a
checked-in baseline against a freshly built manifest and reports one of four
verdicts:

| Verdict | Meaning |
|---|---|
| unchanged | the interface is identical; if the build fingerprint moved, it says so |
| compatible | symbols were added and nothing existing changed |
| breaking | an existing symbol, type, layout, or contract changed |
| target mismatch | the two manifests describe different targets |

A break is accepted only with `--allow-breaking`, which says *this break is
intended* and belongs in the command that ran rather than in a file someone
edited. A target mismatch is never accepted by that flag: nothing about the
source changed, so treating it as an intended break would send a reader looking
for a change that is not there.

Both commands read written manifests and nothing else — no package root, no
compilation — because an installed prefix and an extracted release archive are
exactly where a consumer needs to ask what a library's C surface is, and
neither has a source tree. Reading recomputes both recorded fingerprints, so a
hand-edited manifest is refused rather than compared.

`fol tool abi package --prefix <DIR> --out <ARCHIVE>` turns an installed prefix
into a release archive carrying the headers, libraries, manifests, a
`CHECKSUMS.sha256` any consumer can verify with `sha256sum -c`, a `PROVENANCE`
record, and an `SBOM` naming the pinned components that measured the layouts.
It refuses a prefix containing generated Rust or a Cargo manifest rather than
filtering them out: that is implementation, not interface, and a prefix holding
it is a bug worth reporting rather than papering over.
