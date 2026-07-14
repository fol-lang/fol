# Editor Sync

This document is the canonical contract for keeping the compiler, LSP, and
tree-sitter assets aligned.

## Intent

The editor layer should not become a second language implementation, and it
should not depend on copied compiler name lists.

Current editor non-goals:

- no language/editor claims beyond the checked-in V2 and V3 example matrices
- no editor-owned semantic divergence from compiler results
- no broad rename outside the currently documented safe classes
- no `textDocument/rangeFormatting` until structure-safe partial formatting
  exists
- no claims that code actions cover more than the currently shipped exact
  replacement inventory

The intended split is:

- compiler crates own semantic truth
- `fol-editor` reuses compiler analysis whenever possible
- tree-sitter grammar remains hand-authored
- repetitive editor registries should be compiler-derived
- drift should fail tests

Current shipped V2-aware coverage is intentionally narrow:

- diagnostics, hover, definition, symbols, and completion are exercised against
  the checked-in generic-routine, generic-type, constrained-generic, and
  protocol-standard example packages
- the current positive executable example roots covered by editor tests are:
  - `examples/generic_type_exec_m1m2`
  - `examples/generic_standard_constraint_m1m2`
  - `examples/standards_protocol_m2`
- those tests should stay honest about current boundaries and must not imply
  lowering/backend support that the compiler does not yet ship

Current shipped V3 coverage is compiler-backed and inventory-driven:

- memory examples cover ownership, borrowing, owned allocation, typed pointers,
  `dfr`, and `edf`
- processor examples cover spawn, channels, `select`, `[mux]`, `async`, and
  `await` under hosted `std`
- positive V3 examples participate in LSP open/navigation and tree-sitter
  sweeps; checked-in `fail_mem_*` and `fail_proc_*` examples participate in the
  guarded failure inventory
- semantic diagnostics should flow from compiler truth, while completion and
  tree-sitter captures still require explicit V3 regression coverage

### V3 mirror matrix

V3 completeness is end to end. The rows below name the explicit tooling mirror
that must accompany each semantic slice; compiler acceptance by itself is not a
complete row.

| V3 slice | Compiler/runtime contract | Explicit tooling mirror | Canonical syntax/inventory guard |
|----------|---------------------------|-------------------------|----------------------------------|
| M1 ownership | owned allocation, move/clone selection, reinitialization, control-flow joins, and lexical drops | structured `O1xxx` diagnostics, LSP state/navigation, semantic tokens, and positive-example formatter sweep | `v3_ownership.txt` plus the M1 positive/failure inventory |
| M2 borrowing and cleanup | lexical shared/mutable borrows, give-back, borrow parameters, ownership-aware `dfr`, and error-only `edf` | related-site ownership diagnostics, borrow/deferred hover and navigation, completion filtering, and formatter preservation | `v3_ownership.txt`, `v3_deferred.txt`, and the M2 inventory |
| M3 pointers | typed unique/shared pointers, allocation gating, dereference/write rules, recursion, and place-projection boundaries | model-aware type completion, pointer hover/type-definition, semantic tokens, and exact failure diagnostics | `v3_pointers.txt` plus the M3 inventory |
| P1 spawn | direct named task targets, thread-boundary transfer, and join-at-exit | hosted-tier completion, spawn hover/navigation, and structured direct-target/thread-boundary diagnostics | `v3_eventuals.txt` plus the P1 inventory |
| P2 channels | direct endpoint ownership, send/pull/iteration, capture, and close lifecycle | endpoint-only completion, lifecycle-aware completion, endpoint hover/navigation, and guarded failure diagnostics | `v3_channels_select_mutex.txt` plus the P2 inventory |
| P3 select and mutex | source-order multi-arm selection, optional default, `[mux]` guards, forwarding, and deferred-effect boundaries | select-binding navigation, mutex-method completion, deferred-scope suppression, semantic tokens, and dead-form rejection | `v3_channels_select_mutex.txt` plus the P3 inventory |
| P4 eventuals | internal move-only eventuals, direct async targets, single await, and synchronous error transparency | async/await hover and context completion, navigation, type-shell handling, and tier/boundary diagnostics | `v3_eventuals.txt` plus the P4 inventory |
| Cross-cutting lexical/tooling | compiler-recognized comments and raw strings protect V3 sigils and braces | shared formatter/source scanning, UTF-16 LSP positions, real in-process parse/highlight/symbol commands, and generated-query validation | `v3_lexical_boundaries.txt`, `make tree-test`, and editor integration tests |

Frontend and editor analysis must derive the active model and bundled-standard
aliases from the evaluated artifact contract. A mixed or conditionally hosted
workspace must not gain processor completion, imports, or diagnostics from a
different artifact or an inactive dependency declaration.

The exact processor directory matrix is centralized in the
[shipped processor inventory](../book/src/900_processor/_index.md#shipped-example-inventory).
Compiler integration and editor consumers share the corresponding machine
inventory in `test/v3_example_inventory.rs`; do not reintroduce independent
hard-coded example lists in individual LSP test modules.
In particular, editor diagnostic coverage must preserve the current hard
boundaries for unique-pointer field dereference, deferred reinitialization of a
moved owner, indirect spawn/async call targets, channel endpoint lifecycle, and
deferred mutex field/guard/forwarding effects. These are compiler-owned semantic
rules: the LSP publishes their structured diagnostics, while tree-sitter only
validates and highlights the syntax that reaches them.

## Ownership

### Compiler-owned editor data

These facts should come from compiler crates or compiler-owned registries:

- declaration keyword names
- builtin type names
- container type names
- shell type names
- source kind names
- implemented intrinsic names
- capability facts tied to `fol_model`

### Generated editor data

These should be generated or assembled from compiler-owned data where possible:

- highlight regex fragments for builtin names
- highlight regex fragments for implemented intrinsic names
- completion source lists for builtin names
- completion source lists for implemented intrinsics

### Manual editor data

These are the intentional manual surfaces that remain after sync automation:

- tree-sitter grammar structure
- highlight capture structure
- locals query structure
- symbols query structure
- LSP UX details such as ranking and presentation

## Registry Audit

The current editor registry surface is split like this.

### Must stay manual

These encode editor UX or structural syntax intent, not language-name facts:

| Area | Location | Why it stays manual |
|------|----------|---------------------|
| tree-sitter grammar structure | `lang/tooling/fol-editor/tree-sitter/grammar.js` | grammar shape is structural and cannot be usefully derived from compiler registries |
| structural highlight captures | `lang/tooling/fol-editor/queries/fol/highlights.base.scm` | capture layout is editor-facing presentation logic |
| locals/symbols query structure | `lang/tooling/fol-editor/queries/fol/locals.scm`, `lang/tooling/fol-editor/queries/fol/symbols.scm` | scope/symbol capture shapes are structural tree-sitter authoring |
| completion ranking and tie-breaking | `lang/tooling/fol-editor/src/lsp/completion_helpers.rs` | ordering and UX priority are editor policy |
| semantic token kind mapping | `lang/tooling/fol-editor/src/lsp/semantic.rs` | token categories are an editor-facing legend, not a compiler registry |

### Should be compiler-backed

These are language-name families and should not stay duplicated:

| Area | Location | Canonical source |
|------|----------|------------------|
| builtin type suggestions | `lang/tooling/fol-editor/src/lsp/semantic.rs` | `fol_typecheck::editor_builtin_type_names()` |
| container/shell type suggestions | `lang/tooling/fol-editor/src/lsp/semantic.rs` | `fol_typecheck::editor_container_type_names()`, `fol_typecheck::editor_shell_type_names()` |
| dot intrinsic completion names | `lang/tooling/fol-editor/src/lsp/semantic.rs` | `fol_typecheck::editor_implemented_intrinsics()` |
| `fol_model` availability filtering | `lang/tooling/fol-editor/src/lsp/semantic.rs` | `fol_typecheck::editor_intrinsic_available_in_model()` and `editor_type_family_available_in_model()` |
| command summaries for source kinds and intrinsic families | `lang/tooling/fol-editor/src/commands.rs` | compiler-owned editor metadata |

### Can become generated or centrally assembled

These should be rendered from one canonical helper instead of repeated string
lists:

| Area | Location | End state |
|------|----------|-----------|
| checked-in `highlights.scm` name families | `lang/tooling/fol-editor/src/tree_sitter.rs`, `queries/fol/highlights.scm` | generated from compiler metadata |
| command summary detail strings | `lang/tooling/fol-editor/src/commands.rs` | assembled from shared metadata helpers |
| editor sync regression snapshots | `lang/tooling/fol-editor/src/tree_sitter.rs`, `test/run_tests.rs` | compare against canonical rendered metadata instead of copied strings |

### Intentional leftovers after this plan

If a registry is still manual after the plan completes, it should be manual for
one of these reasons:

- it defines tree-sitter structure, not a compiler name family
- it defines editor ranking or rendering policy
- it describes a token/UX vocabulary that is intentionally editor-owned

## `fol_model` contract

The editor must treat `fol_model` as a real semantic boundary.

That means:

- diagnostics shown by LSP should match compiler/build diagnostics
- completion should hide surfaces that are invalid for the active model
- mixed-model workspaces should not silently bleed one model into another

## Capability matrix

| Capability mode | Bundled std declared | Type completion | Intrinsic completion | Diagnostics focus | Example packages |
|-----------------|----------------------|-----------------|----------------------|-------------------|------------------|
| `core` | no | scalars, arrays, records, entries, shells, and analyzable `ptr[...]` types | no hosted or heap-only guidance | allow `core` execution; enforce ownership/borrowing; reject pointer construction, owned allocation, `str`, dynamic containers, processor surfaces, bundled std imports, and `.echo(...)` | `examples/core_run_min`, `examples/core_blink_shape`, `examples/core_dfr`, `examples/core_records`, `examples/core_surface_showcase`, `examples/fail_core_std_import` |
| `memo` | no | `core` types plus `str`, `vec`, `seq`, `set`, `map`, and allocating pointer/owned forms | no bundled std wrappers or processor intrinsics | allow `memo` execution and memory-pillar behavior; reject bundled std imports, processor surfaces, and `.echo(...)` | `examples/memo_run_min`, `examples/memo_defaults`, `examples/memo_containers`, `examples/memo_collections`, `examples/memo_surface_showcase`, `examples/mem_ptr_unique_m3`, `examples/fail_memo_echo`, `examples/fail_memo_std_missing_dep` |
| `memo` | yes | `memo` types plus bundled `std` package exports, channels, and mutex-aware routine surfaces | bundled `std`, hosted runtime behavior, `[mux]` operations, and processor guidance | ordinary semantic/type diagnostics plus spawn/channel/select/mutex/eventual boundary checks | `examples/std_bundled_fmt`, `examples/std_bundled_io`, `examples/proc_spawn_m1`, `examples/proc_channel_m2`, `examples/proc_select_m3`, `examples/proc_async_await_m4` |

For mixed-model workspaces, editor tests should also cover
`examples/mixed_models_workspace`.

All three rows may describe executable artifacts. Bundled std changes the APIs
that compiler-backed editor analysis exposes; it does not decide whether the
frontend may run or test a host-compatible artifact.

## Routed artifact fallback

When the editor can map an opened file to one routed artifact root from
`build.fol`, it should use that artifact's `fol_model`.

When the file does not map to one specific routed artifact:

- if every routed artifact in the package uses the same `fol_model`, the editor
  should reuse that uniform package model
- if routed artifacts disagree, the editor should keep the model unknown rather
  than guessing

That keeps mixed-model packages deterministic and avoids silently bleeding one
artifact model into unrelated helper files.

Editor-facing expectations:

- real transitive model-boundary failures should surface through LSP
- files mapped to a single artifact should use that artifact's exact
  `fol_model`
- ambiguous package-local files should stay conservative and avoid bleeding a
  narrower model into unrelated helpers
- build files that declare `fol_model = "core" | "memo"` should stay
  discoverable in semantic token and tree-sitter coverage
- negative model examples such as `examples/fail_core_heap_reject` and
  `examples/fail_memo_echo` should keep surfacing the same LSP boundary class

## Test gates

The minimum test gates for editor sync are:

- compiler constants match tree-sitter query name families
- compiler intrinsics match editor highlight/completion name families
- model-boundary diagnostics match between LSP and build-mode compilation
- real example packages for `core`, `memo`, and bundled-std-backed `memo` stay
  editor-readable
- every positive V3 example stays in LSP and tree-sitter inventory sweeps
- every checked-in V3 failure example stays in the guarded diagnostic inventory
- nested V3 type operands and declaration sigils retain exact tree-sitter
  captures, and `[mux]` receivers expose only their legal mutex operations
- every positive V3 source remains formatter-idempotent and compiler-analyzable
- artifact-scoped LSP analysis uses evaluated active dependencies rather than
  statically declared or neighboring artifact capabilities
- UTF-16 positions and related diagnostic URIs remain valid for non-ASCII files

## Contributor rule

If a language feature changes semantic behavior only:

- the editor should usually pick it up through compiler-backed analysis

If a language feature adds new names:

- update the compiler-owned registry
- generated editor surfaces should follow from that

If a language feature changes syntax shape:

- update tree-sitter grammar and structural queries
- keep the manual surface small and test-guarded

## Contributor Checklist

When you add or change a language feature, the editor sync bar is:

1. Update compiler-owned metadata first if the feature adds:
   - declaration heads
   - builtin type families
   - implemented intrinsic names
   - `fol_model` capability boundaries
2. Run the editor sync tests and keep them green:
   - compiler/query sync tests
   - top-level editor sync integration tests
   - model-aware LSP completion and diagnostics tests
3. If the feature changes only semantic behavior:
   - do not add a duplicated editor-only semantic rule first
   - prefer the compiler-backed analysis path
4. If the feature adds only names or registries:
   - generation or compiler-backed helpers should cover the editor surface
   - avoid hand-editing duplicate completion/highlight name lists
5. If the feature changes syntax shape:
   - update `tree-sitter/grammar.js`
   - update structural query files such as `highlights.base.scm`, `locals.scm`,
     or `symbols.scm`
   - keep structural edits minimal and covered by tests
6. If the feature changes `fol_model` legality:
   - add or update `core` / `memo` editor example coverage
   - verify LSP diagnostics match `fol code build`
7. If the feature adds or changes bundled `std` public names:
   - update bundled std examples that should demonstrate the new names
   - add or update LSP completion plus hover/definition coverage
   - add or update tree-sitter real-example highlight coverage

The intended workflow is:

- compiler change first
- generated/compiler-backed editor surfaces update next
- manual tree-sitter edits only when syntax structure changed
- `make build` and `make test` stay green before merge
