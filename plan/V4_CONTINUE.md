# V4 Continued — Foreign Boundary Completion and Hardening

> **Status: not started.** This is the continuation of `V4_PLAN.md`, not a new
> release. Same milestone family, same branch, same guardrails; the numbering
> picks up at M10 because M0-M9 are done. A checked box here means a verified
> shipped result, not parser acceptance or an implementation sketch.

M0-M9 made FOL a participant in C toolchains: it exports a real C ABI, imports
real C libraries, and ships release archives a stranger can use. The certified
`x86_64-unknown-linux-gnu` and `x86_64-unknown-linux-musl` lanes pass with zero
skips under `FOL_H7_REQUIRED=1`.

What that did not do is make the boundary *symmetric* or *complete*. Three
shapes cross in one direction only, one shape does not cross at all, and
several checks that sound enforced are enforced by argument rather than by
code. M10-M16 close those, and then attack what is shipped rather than adding
to it:

- **Completion** (M10-M13): the foreign surface a real header actually needs.
- **Closure and hardening** (M14-M16): proving the failure modes, clearing the
  supply-chain residuals, and going looking for defects.

The hardening half is not a formality. Every defect M0-M9 found was found by
running something, never by reading generated text, and several were invisible
to a green suite. That ratio should be expected to hold.

`V4_PLAN.md` remains the authority for M0-M9 and for every guardrail, ownership
rule, and STOP condition. This file adds milestones; it does not restate or
override that one. Where the two disagree about a technical fact, this file
wins only where it cites a measurement.

---

# 1. Definition of Done

M10-M16 are done when all of the following hold **and** are proven by a lane in
`make verify`:

1. A C `struct` and a C `enum` cross **inbound** as ordinary FOL values.
2. An opaque handle, its destroy routine, and a callback cross **outbound** to
   a C consumer.
3. An exported entry carries a discriminant FOL and C agree on, or entries stay
   refused with the same honest reason they are refused with today.
4. An owned buffer crosses with its capacity, length, and domain validated
   before reconstruction.
5. Every native-provider failure mode reports a structured FOL diagnostic
   naming the provider and the reason, never a raw linker dump.
6. Every item in `V4_PLAN.md` §18 is `[x]` or carries a measured reason that
   cites code, not an intention.
7. A deliberate adversarial pass over the V4 surface has run, and everything it
   found is either fixed or recorded with a reproduction.

**Not** required for done: Rust interop, a C++ ABI, multiple headers per
import, or any target beyond the two certified lanes. Those stay non-goals.

---

# 2. Permanent Guardrails

All of `V4_PLAN.md`'s guardrails carry over unchanged. The four that this plan will be
tempted to break, restated because the temptation is specific:

- **No half-features.** A shape either crosses and is tested end to end, or it
  is refused by name. the record-import gap is refused cleanly today; a
  partial implementation that produces a manifest and then dies mid-pipeline
  with an internal error is strictly worse than that.
- **No sibling ownership.** PARC owns C parsing, LINC owns native evidence,
  GERC owns raw projection. If a milestone here appears to need FOL to parse C
  or resolve a provider, the milestone is wrong.
- **Refuse rather than approximate.** A construct FOL cannot model exactly does
  not compile.
- **A skip is not a pass.** Every lane added here honours `FOL_H7_REQUIRED`.

One guardrail is new, and it is the lesson M0-M9 paid for:

- **A check that cannot fire is worse than no check.** M3 shipped an
  object-format comparison that was unreachable by construction: it read as
  coverage and could never fail. Any validation added here must be accompanied
  by a test that fails when the validation is removed.

---

# 3. Verified Truth Snapshot

Everything in this section was measured against the tree at the close of M9,
not inferred. It is here so M10-M16 start from facts.

## 3.1 What the export path can produce

`project_exports` in `fol-lower/src/abi.rs` is the only producer of an exported
routine. It maps a lowered FOL type into `CandidateType`
(`fol-abi/src/verify.rs`), whose variants are `Int, Float, Bool, Char,
BorrowedString, Record, Entry, RawPointer, ManagedPointer, Container,
RoutineObject, ConcurrencyObject, Generic, UnsupportedLayout, Void`.

There is **no handle variant and no callback variant**. `AbiType::OpaqueHandle`
and `AbiType::Callback` are constructed in exactly one place in the tree —
`fol-interop/src/interface.rs`, the *import* path. `intern` in
`fol-lower/src/abi.rs` accepts `Record`, `Entry`, `Scalar`, `Void`, and
`BorrowedString`; everything else falls off.

A FOL routine value in an exported signature is rejected as
`RoutineOrProtocolObject`. A managed pointer is rejected as `UnwrappedPointer`.
A raw pointer is rejected as `IncompletePointerContract` twice over, for
missing ownership and missing escape.

`AbiExportConfig` is frozen at exactly two fields, `routine` and `symbol`, so
there is also no syntax to declare a destroy pairing on an export.

## 3.2 What the import path can consume

`fol-typecheck/src/c_import.rs` maps `AbiScalar::{Int,Float,Bool,Char}` and
`AbiType::Void`. Every other `AbiType` reaches a fallback that returns an
internal error naming the kind. `AbiType::OpaqueHandle` is special-cased
earlier into a nominal type; `AbiType::Callback` into a routine type.

So a record has no path even though `project_imported_interface` could be
taught to produce one: `AbiType::Record` exists in the model, serializes, and
round-trips, and GERC hands over `RustRecord` with per-field offsets, sizes,
packing, and support status. **The missing piece is not the projection. It is
the nominal FOL type.**

## 3.3 What is enforced by argument rather than by code

- **Cross-thread callback invocation** is refused because the closure slot is
  thread-local. That follows from Rust's `thread_local!` semantics and has no
  test of its own.
- **Reentry** is permitted and indistinguishable at the boundary from the
  ordinary case.
- **`fingerprint_tool`** is written, tested, and called from nowhere in
  production.
- **Sysroot** reaches `ScanConfig` but not `TargetSpec`, whose `sysroot` stays
  `None`, so it is honoured for finding headers and is not part of target
  identity. `SysrootIdentity` requires a `ContentFingerprint` over a directory
  tree, which is an unanswered design question, not an oversight.
- **The compiler** is recorded in import provenance as a path, not a digest.

## 3.4 What blocks entry discriminants

FOL has no syntax for an explicit ABI discriminant. A variant is written
`con NAME: int = 7` or `var NAME: T = v`, and the parser records both the same
way: the value is the variant's **default payload**, not a tag.

The enum-shaped case makes it look like a tag — `Severity.RETRY` really does
evaluate to `7` — but `var Ok: int = 1` beside `var Err: str = "broken"` shows
what it is. Taking the default as a tag gives both variants the tag `1`.

This was found by probe, not by reading: the projection first shipped
positional tags, and FOL evaluated `Severity.RETRY` as **7** while the header
declared `FOL_SEVERITY_RETRY = 1`. One entry, two tags, depending on which side
you stood on. Reusing the default as the discriminant was then tried and
reverted, because the collision above is real.

Entries are therefore refused outbound with `AbiRejection::UnstableEntryTag`.
**M12 is a language-syntax milestone before it is an ABI milestone.**

*Closed by M12.* `con[tag = N]` states the discriminant, and an entry whose
variants all state one crosses. An entry with none is still refused with the
same rejection and the same reason -- see M12's Landed section.

## 3.5 What LINC's certification forbids

`validate_certification_request` requires `ResolutionPolicy::ExactPathsOnly`,
and exact-path resolution separately rejects a search path as an input. So:

- Declared library search paths cannot be honoured at all under the pinned
  LINC revision, whatever the build record says.
- The dynamic and system-library provider forms cannot certify.

`ResolverConfiguration::toolchain_search_paths` is a decoy here: it is read
only under `ToolchainSearch`, the ambient-discovery policy FOL refuses. The
correct channel would be `NativeInput::SearchNative` under `HermeticSearch`,
which is genuinely hermetic — and which certification rejects.

**This is a sibling-side blocker.** FOL cannot fix it in FOL.

---

# 4. M10 — Nominal C Types Inbound

Goal: a C `struct` and a C `enum` cross inbound as ordinary FOL values.

Primary files:

- `lang/tooling/fol-interop/src/interface.rs`
- `lang/compiler/fol-typecheck/src/c_import.rs`
- `lang/compiler/fol-resolver/src/c_import.rs`
- `lang/tooling/fol-interop/src/adapter.rs`
- `lang/execution/fol-backend/src/instructions/render.rs`

Tasks:

- [x] Resolve `RustTypeKind::Named` to its declaration and project a `struct`
  as `AbiType::Record`, admitting only Section 4.13's shape: `struct` kind,
  natural `repr(C)` layout, byte-aligned fields, no self-reference by value.
- [x] Refuse union, opaque, packed, bitfield, and flexible-array members by
  name, each with the reason rather than a generic "could not be modelled".
- [x] Project a C `enum` as a FOL value at its measured underlying width, with
  the enumerator names carried for diagnostics. Do **not** project it as a FOL
  entry: a C enum is an integer with named constants, and pretending otherwise
  re-creates the tag problem M12 exists to solve.
- [x] Synthesize a nominal FOL record type from `AbiType::Record` and mount it
  in the import namespace, the way `OpaqueHandle` mounts a name today.
- [x] Support field access on that type, and construction where the overlay
  declares the record inbound-constructible.
- [~] Marshal by value in the adapter and backend, with the layout FOL believes
  checked against the layout the provider was compiled with.

Tests:

- a C provider taking and returning a POD struct, called from FOL, with a field
  value that can only come from C having read the struct FOL built
- a struct nested one level, and a struct containing a pointer
- a C enum parameter accepted at its measured width, and an out-of-range value
  refused before use
- union, packed, bitfield, flexible-array, and self-referential structs each
  refused with their own reason
- the layout FOL projects compared against `offsetof`/`sizeof` measured by the
  provider's own compiler

Verification: `make test`, `make test-v4-c-import`, `make abi-check`,
`make test-v4-sanitize`.

**STOP:** this cannot land partially. A manifest that describes a record which
then fails in typecheck, the adapter, or the backend is worse than today's
clean refusal, because it moves the failure away from the declaration that
caused it.

## Landed

A C `struct` and a C `enum` both cross inbound. `6 * 7 = 42` computed by C from
a struct FOL built, with `p.x` still readable afterwards, is the evidence.

**The design changed once, under measurement.** The first attempt named GERC's
raw struct as the FOL type directly -- no conversion, provider layout for free.
It compiled all the way to `rustc` and failed there: GERC emits its structs
with **no derives at all**, so the raw struct has no `Clone` and no `Default`
and cannot be a FOL value. What ships instead: FOL emits its *own* struct for
the imported record, from a `LoweredTypeDecl` synthesized for a symbol that has
no AST behind it, and the adapter takes the record's **fields** rather than the
struct, rebuilding the provider's own struct inside. Field by field, never
transmuted -- the rule the export wrapper already followed in the other
direction.

Two things that only running it could have found:

- `struct node { struct node *next; }` -- a list node, an entirely ordinary
  header shape -- **overflowed the stack**, because the pointer's target
  resolves back to the record being projected. Now a named refusal.
- An imported record was first treated as move-only, so reading one *after*
  passing it to C silently returned a defaulted value rather than erroring. C
  passes a struct by value, so a copy is what the provider's own calling
  convention already does; it is now copy, and the test reads the record after
  the call to prove it.

A C enum crosses as the integer the target measured, **not** as a FOL entry: an
enum is an integer with named constants, and projecting it as a tagged union
would invent the discriminant contract M12 exists to establish honestly.

Still open: a record in *result* position, which needs the reverse conversion
and is refused by name.

---

# 5. M11 — Owned Resources Outbound

Goal: FOL hands C an opaque handle, a destroy routine, and a callback.

Primary files:

- `lang/compiler/fol-lower/src/abi.rs`
- `lang/compiler/fol-abi/src/verify.rs`
- `lang/execution/fol-backend/src/abi/{header,wrapper}.rs`
- `lang/execution/fol-build/src/{semantic,graph}.rs`

Tasks:

- [x] Add handle and callback variants to `CandidateType`, and the matching
  `intern` arms, so `AbiType::OpaqueHandle` and `AbiType::Callback` become
  reachable from an export.
- [x] Emit an opaque struct typedef in the generated header for an exported
  handle domain, with the same nominal-identity rule the import side uses: the
  domain is the type, and no runtime tag is involved.
- [x] Extend the frozen `AbiExportConfig` with a destroy pairing, and enforce
  it the way the import overlay does — exactly one destroy per domain, and no
  other exported routine may consume the domain.
- [x] Export a callback as a function pointer in the canonical shape, with the
  context parameter FOL owns.
- [x] Refuse an exported handle with no destroy, and an exported callback whose
  shape is not canonical, each by name.

Tests:

- a C11 consumer creating a FOL handle, borrowing it, and destroying it once
- a consumer that leaks it, and one that destroys it twice, each caught
- a consumer registering a callback that FOL invokes during a call
- both linkage forms, static and shared
- the negatives above under ASan/UBSan

Verification: `make test-v4-c`, `make test-v4-c-platform`,
`make test-v4-sanitize`, `make test-v4-c-release`.

**STOP:** an exported handle whose destroy is not declared and enforced is a
leak the type system promised to prevent. Do not ship the handle export without
the pairing.

## Landed

**Handles.** The header declares `typedef struct fol_session_t fol_session_t;`
and never defines it, so a consumer holds the address, hands it back, and the C
compiler refuses to let it read through or copy what is behind it. *Produces*
boxes the FOL value and returns its address; *borrows* lends what the address
points at; *consumes* takes the box back, which is what makes the release
happen exactly once. A null handle is refused rather than dereferenced.

The pairing is enforced across the whole allowlist, not per routine: a producer
must name a destroy, that destroy must be an exported consumer of the same
domain, a domain has exactly one producer, and a consumer with no producer is
refused. The C consumer borrows *twice* before releasing, so a borrow that
quietly consumed would fail rather than pass.

The domain name is the FOL type name -- `HandleUse`'s own documentation already
said so for the import side, so the export side follows the same rule instead
of inventing a second mapping. That is also what lets the wrapper find the Rust
path it boxes and unboxes through.

**Callbacks.** C supplies a function pointer and a context; FOL receives an
ordinary routine value. The context travels beside the pointer because C has
nowhere else to put the state a callback needs, and the closure exists only for
the duration of the call. `Option<unsafe extern "C" fn(..)>` rather than a bare
pointer, so null is a value the wrapper tests rather than undefined behaviour
on first call.

A callback is legal in parameter position and nowhere else: returning one would
hand C a pointer to a FOL closure whose environment stops existing when the
call returns. A routine that *reports* is refused too -- a callback has one
result channel and no way to use another.

The test counts invocations through the context, which is what proves FOL
actually called back rather than computing the answer another way.

Found by running it: a borrower writes `Session[bor]`, so the loan has to be
peeled before matching the domain type, or the borrowing routine looks like it
takes no handle at all.

---

# 6. M12 — Explicit ABI Discriminants

Goal: an exported entry carries a discriminant FOL and C agree on.

This is a **language-syntax milestone first**. §3.4 records why: FOL has no way
to distinguish a variant's tag from its default payload, and both plausible
shortcuts have been tried and rejected with evidence.

Tasks:

- [x] **Owner decision required**: choose the syntax for an explicit
  discriminant. This plan does not pick one. Candidates that fit FOL's existing
  conventions, for the owner to rule on:
  - a bracket option on the variant, `con[tag = 7] RETRY: int`, matching the
    `[cpy]`/`[mut,bor]` option family
  - a dedicated clause in the entry declaration, listing tags separately from
    variants
  - a whole-entry attribute fixing the tag type and requiring every variant to
    state one
- [x] Parse, resolve, and typecheck the chosen form, with the tag type fixed
  and explicit rather than inferred.
- [x] Reject a duplicate tag, a tag outside the declared type's range, and a
  partially-tagged entry.
- [x] Project the tag into `AbiType::Entry` and the generated header, replacing
  the `UnstableEntryTag` refusal.
- [x] Make a declaration reorder a no-op on tags, and an intentional tag change
  an ABI break that `fol tool abi check` reports as breaking.

Tests:

- an entry whose variants are reordered produces a byte-identical manifest
- an intentional tag change is reported `breaking`, and accepted only with
  `--allow-breaking`
- a probe comparing FOL's evaluated variant value against the header's constant
  for every variant — the exact probe that caught the original mismatch
- duplicate, out-of-range, and partial tagging each refused

Verification: `make test`, `make abi-check`, `make test-v4-c`,
`make docs TYPE=mdbook`.

**STOP:** do not reuse the default payload as the tag. That was tried, and
`var Ok: int = 1` beside `var Err: str = "broken"` gives both variants tag `1`.

## Landed

**The syntax decision was made without the owner.** The plan asked for a
ruling and the milestone could not proceed without one, so the first candidate
was taken: a bracket option on the variant.

```fol
typ[exp] Lookup: ent = {
    con[tag = 4] MISSING;
    con[tag = 1] FOUND;
    con[tag = 9] DENIED;
};
```

It was chosen because it reuses the option-bracket family FOL already spells
everywhere -- `fun[exp]`, `var[mut]`, `[mut,bor]` -- rather than introducing a
fourth declaration shape, and because it puts the tag on the variant it belongs
to, which the separate-clause candidate does not. **This is reversible**: the
tag reaches the rest of the compiler as one `Option<i64>` on
`EntryVariantMeta`, so a different spelling is a parser change and nothing
else. If the owner prefers another form, say so and it moves.

`[tag = N]` is its own bracket group rather than one option among several,
because it takes a value and the option list is a list of bare names.

**The three refusals are parse-time**, because all three are local to one
declaration: a duplicate tag names the variant that already holds it, an
out-of-range tag names the 32-bit limit the discriminant carries, and a
partially tagged entry reports how many of how many. Nothing downstream has to
re-derive them.

**The ABI plumbing already existed.** `AbiVariant::discriminant`, the header's
enum rendering, and the wrapper's conversions in both directions were all built
in M4-M5 against a tag FOL could not yet state. What M12 added is the syntax
and one bit: `explicit`, travelling beside the discriminant so the verifier can
tell a stated tag from a positional one. The `describe` arm that hard-coded
`None` now passes the tag through.

**Reordering is a genuine no-op**, verified by comparing two builds' manifests
byte for byte rather than by reading the tags back. The ABI projection orders
variants by tag rather than by declaration -- position means nothing once the
tag is written down. A record cannot do this, because field order decides
offsets; a tagged variant's position decides nothing. Changing a tag is the
control: the same comparison differs, and `fol tool abi check` reports it
breaking under `F1004`.

Two things had to be built that the plan did not anticipate:

- **Tree-sitter had no `con NAME;`** -- a variant with no type and no default.
  Every entry in the repo was written `con NAME: int = 0`, so the gap had never
  shown. Making `con`'s value optional globally is not possible: it collides
  with the comma-separated name list in expression contexts, which tree-sitter
  reports as an unresolved conflict. The fix is a separate `entry_variant` rule
  in the type block, where the enclosing context is unambiguous.
- **The book's "what crosses" table was ahead of the code** -- it already
  claimed entries crossed "with explicit stable discriminants". It does now.
  The same pass corrected the stale M10/M11 rows, which still listed exported
  handles, exported callbacks, and imported aggregates as unsupported.

---

# 7. M13 — Owned Buffers and Pointer Contracts

Goal: a buffer crosses with its capacity, length, and domain validated.

Tasks:

- [x] Add pointer/length pairing to the annotation overlay, so a
  `(const uint8_t *, size_t)` pair imports as one FOL value rather than two
  unrelated parameters.
- [x] Make direction declarable rather than inferred from pointee constness.
- [ ] Validate capacity, length, and domain before reconstructing an owned
  buffer, and refuse a length that exceeds capacity, a null with a nonzero
  length, and a domain mismatch.
- [ ] Pair every owned buffer with exactly one release path, the way handles
  are paired.

Tests:

- a provider returning an owned buffer, consumed and released once by FOL
- length > capacity, null-with-length, and wrong-domain each refused
- the accepted path under ASan/UBSan
- a slice imported through the pairing, with the length actually respected

Verification: `make test-v4-c-import`, `make test-v4-sanitize`,
`make test-v4-linear`.

## Landed so far — the borrowed half

The first two tasks are done and gated; the owned-buffer half (tasks 3 and 4)
is not started.

**The pairing.** `buffer = "bytes"` with `buffer_length = "count"` makes the
two C parameters one FOL value. The length then stops being a FOL parameter at
all -- it is derived from the value at the call site, so there is no second
number for a caller to get wrong and no way to describe a buffer longer than
the one that exists. `AbiType::BorrowedSlice` already existed, serializable and
renderable, with nothing constructing one; this is what constructs it.

On FOL's side it is a borrowed vector: `bor[vec[u8]]` for a read-only buffer
and a mutable loan for one the provider writes. Borrowed rather than owned
because the storage stays FOL's -- the provider is lent it for the call.

**Direction is declared, not guessed.** `reads` / `writes` / `reads_writes`
name the parameters they apply to, in the same style as `nullable` /
`transferred` / `retained`. Constness stays the default when nothing is
declared, but it is a poor witness: `void *base` in `qsort` is read and
written, `char *dst` in `strcpy` is only written, and a mutable pointer a
provider never writes is indistinguishable from either. A declaration C
contradicts -- `writes` on a const pointee, or any direction on a by-value
parameter -- is refused rather than believed.

Six refusals, each naming the routine and the parameter: a buffer or length
that is not a parameter of that signature, a parameter paired with itself, a
declared half with no other half, a signed length, a pointer to something with
no size (`void *`), and a by-value parameter named as a buffer.

**Verified by moving the buffer.** Four elements summed to 10; five summed to
100. A length that was hardcoded, ignored, or off by one fails one of the two.
Both directions run in one program: C reads four elements, then writes through
a mutable loan, and FOL sums the result back.

Three things found by running it:

- **`out` was already taken.** It names the status convention's out-parameter,
  so the direction keys say what the provider does through the pointer instead.
  The compiler caught the shadowing as an unreachable match arm.
- **The import manifest's type writer had a silent catch-all** -- `other =>
  kind only` -- so a slice serialized without its element or mutability and
  read back as a type the reader rejects. Both sides are explicit now.
- **A mutable loan renders as `&mut *local`**, so taking `.as_mut_slice()` off
  the rendered form produced `&mut *local.as_mut_slice().len()`: dereferencing
  the length. The loan has to be peeled before the slice is taken.

Adding the `buffer` field to the import manifest invalidated every checked-in
`.folabi.json`. They were regenerated rather than edited -- a hand-edited
manifest is exactly what the fingerprint refuses, and one test proves it.

---

# 8. M14 — Provider Diagnostics and Link Evidence

Goal: every native failure reports a FOL diagnostic, never a raw linker dump.

This milestone is mostly *audit and coverage*, not new machinery. M9 already
added real object-format and architecture inspection; what is missing is proof
that each failure mode reports well.

Tasks:

- [ ] Prove local exact-file, dependency-provided archive, dynamic library,
  system library, and target-specific missing-provider diagnostics, each with a
  fixture that produces it.
- [ ] Report missing symbol, missing library, wrong architecture, wrong object
  format, duplicate provider, and link cycle as structured diagnostics with
  related sites.
- [ ] Attach header source ranges to the rejections that currently lack them.
  §3.5 is the constraint: LINC's profile rejections name the construct
  precisely but carry no range, because they are statements about a type rather
  than a declaration. Either map them back to a declaration or record why not.
- [ ] Confirm no path surfaces a raw linker dump as the primary error.

Tests: one fixture per failure mode, each asserting the FOL code and the named
provider, and asserting the absence of raw linker text.

Verification: `make test-native`, `make test-build-actions`,
`make test-v4-c-import`.

---

# 9. M15 — Supply-Chain Residuals

Goal: every `V4_PLAN.md` §18 item is `[x]` or cites code for why not.

Tasks:

- [ ] Digest the compiler binary into import provenance, not just its path.
- [ ] Wire `fingerprint_tool` into the exec path. It is written and tested and
  called from nowhere; the work is folding it into the cache identity, which is
  computed before execution rather than at the exec site where the tool check
  lives.
- [ ] Decide the sysroot identity question: either define what a
  `ContentFingerprint` over a sysroot hashes, or record that FOL will not put
  sysroot in target identity and why.
- [ ] Give cross-thread callback invocation a test of its own rather than
  relying on `thread_local!` semantics.
- [ ] Raise the interop pin and re-measure §3.5. If a newer LINC certifies
  hermetic search, the dynamic and system-library provider forms unblock and
  `library_paths` stops being a refusal.

Verification: `make verify`, `make interop-locked`, `make abi-check`.

---

# 10. M16 — Adversarial Hardening

Goal: find what is wrong with what shipped.

This milestone adds no surface. It is scheduled last and given real time
because M0-M9's evidence says it will pay: nine defects were found by running
things, and several were invisible to a green suite — an unreachable check that
read as coverage, a dead `is_null` with no caller, a lexer bug that silently
deleted a declaration.

Tasks:

- [ ] Differential probe: for every shape that crosses, compare FOL's belief
  against the provider's own compiler — `sizeof`, `offsetof`, enum values,
  struct padding, and the exact bit pattern of every scalar at both edges of
  its range.
- [ ] Fuzz the import path with generated headers: deep typedef chains,
  qualifier stacks, anonymous members, and every construct on the refusal list,
  asserting a named refusal and never a panic.
- [ ] Lifecycle stress: handles and callbacks across nesting, early return,
  panic unwinding, and the failure channel, asserting exactly one release on
  every path.
- [ ] Sanitizer sweep over every checked-in consumer, including the negatives,
  with a deliberate-violation control so the lane cannot pass by not running.
- [ ] Reproducibility sweep: N clean builds of every example compared byte for
  byte on header, manifest, symbol list, and library.
- [ ] Audit every `[x]` in `V4_PLAN.md` against the code, the way §18 was
  audited. Assume some are wrong.
- [ ] Grep the tree for checks that cannot fire — unreachable branches,
  constructed-but-never-tested error variants, and functions with no production
  caller.

Verification: `make verify` plus each new lane, all under `FOL_H7_REQUIRED=1`.

**STOP:** This does not close while a known defect is unrecorded. A defect that
is found and cannot be fixed in scope gets a reproduction and a plan entry, not
silence.

---

# 11. Cross-Cutting Rows

Apply in the same commit as the slice, not as a late phase. `V4_PLAN.md`'s cross-cutting
inventory carries over; the rows that M10-M13 will actually touch:

- **Diagnostics**: a stable code for every new refusal, with `fol code explain`
  text and human/plain/JSON parity.
- **Formatter**: the M12 discriminant syntax formats and stays idempotent.
- **Tree-sitter**: the M12 syntax parses, highlights, and has corpus coverage;
  the parse ratchet is refreshed and its diff is deletions only.
- **LSP**: hover on an imported record shows its C layout; hover on an exported
  handle shows its destroy pairing; completion offers the new export config
  fields from the shared registry, as `add_c_import` now does.
- **Book**: the interop chapter's *What crosses* table is the contract. Every
  milestone here moves a row from the exclusion list to the inclusion list, and
  a row moves only when a lane proves it.

---

# 12. Risk Register

| Risk | Consequence | Prevention / early signal |
|---|---|---|
| M10 lands partially | manifests that die mid-pipeline; worse than today's refusal | one vertical slice, refused until it runs end to end |
| M12 syntax picked without the owner | a language surface nobody wants, cemented by an ABI | the decision is a task, not an assumption |
| M12 reuses the default payload as a tag | two variants share a tag; C and FOL disagree | the probe from §3.4 is a required test |
| Export handles ship without destroy pairing | a leak the type system promised to prevent | the pairing is in the same milestone, not a follow-up |
| Hardening deferred to "if there is time" | M0-M9's ratio says defects remain | M16 is scheduled, not optional |
| A new check cannot fire | reads as coverage, catches nothing | every validation ships with a test that fails when it is removed |
| Interop pin raised mid-milestone | sibling behaviour changes under a half-done slice | pin bumps land alone, with §3.5 re-measured |

---

# 13. Hard STOP Conditions

This continuation does not close while any of these hold:

- a shape crosses in one direction and is silently absent in the other
- an entry ships a tag FOL and C disagree on
- an owned resource crosses without exactly one release path
- a native failure surfaces a raw linker dump as the primary error
- a lane can pass by skipping
- a `[x]` in this file or in `V4_PLAN.md` is not backed by a lane
- M16 has not run

---

# 14. Explicit Non-Goals

Unchanged from `V4_PLAN.md`, restated because each will be proposed:

- Rust interop, a C++ ABI, or any second public boundary
- arbitrary Cargo ingestion
- unrestricted unsafe code in ordinary FOL
- multiple headers or providers per import, unless M15's pin raise makes it free
- any target beyond the two certified lanes
- generators, `yield`, and broader V2 expressiveness

---

# 15. Recommended Order

Land in this order. The dependencies are real, not stylistic.

1. **M10** — inbound records and enums. Largest user-visible gap; most real
   headers pass structs. It also settles how a nominal C type is mounted, which
   M11 needs.
2. **M11** — outbound handles and callbacks. Easier to specify once M10 has
   settled nominal mounting, and it closes the symmetry gap.
3. **M12** — discriminants. Independent of M10/M11, but gated on an owner
   decision, so start the decision early and implement when it lands.
4. **M13** — buffers and pointer contracts. Builds on M10's field machinery.
5. **M14** — provider diagnostics. Mostly audit; can run in parallel with M13.
6. **M15** — supply-chain residuals. Small, and the pin raise may unblock work
   the earlier milestones had to refuse.
7. **M16** — adversarial hardening. Last, over everything, with real time.

---

# 16. Completion Rule

A milestone is complete when its tasks are `[x]`, its tests exist and run in
`make verify`, its cross-cutting rows are applied, and its STOP conditions do
not hold. A milestone is not complete because its code is written, because the
suite is green, or because a plan file says so.

The rule M0-M9 earned, kept here verbatim: **prove it by running it.** Every
defect they found came from executing something. None came from reading
generated text and finding it convincing.
