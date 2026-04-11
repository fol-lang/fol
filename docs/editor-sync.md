# Editor Sync

This document is the canonical contract for keeping the compiler, LSP, and
tree-sitter assets aligned.

## Intent

The editor layer should not become a second language implementation, and it
should not depend on copied compiler name lists.

Current editor non-goals:

- no broad V2-aware language/editor support beyond the shipped generic-routine
  and protocol-standard example subset
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
  the checked-in generic-routine and protocol-standard example packages
- those tests should stay honest about current boundaries and must not imply
  lowering/backend support that the compiler does not yet ship

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
| `core` | no | scalar, array, record, entry, shell surfaces only | no hosted or heap-only guidance | reject `str`, dynamic containers, dynamic `.len(...)`, bundled std imports, `.echo(...)` | `examples/core_blink_shape`, `examples/core_defer`, `examples/core_records`, `examples/core_surface_showcase`, `examples/fail_core_std_import` |
| `memo` | no | `core` types plus `str`, `vec`, `seq`, `set`, `map` | no bundled std wrappers or hosted intrinsics | reject bundled std imports and `.echo(...)`; allow heap-backed strings and containers | `examples/memo_defaults`, `examples/memo_containers`, `examples/memo_collections`, `examples/memo_surface_showcase`, `examples/fail_memo_echo`, `examples/fail_memo_std_missing_dep` |
| `memo` | yes | `memo` types plus bundled `std` package exports under the declared alias | bundled `std` wrappers and hosted-runtime behavior | ordinary semantic/type diagnostics plus dependency-backed std import behavior | `examples/std_bundled_fmt`, `examples/std_bundled_io`, `examples/std_cli`, `examples/std_echo_min`, `examples/std_named_calls`, `examples/std_surface_showcase`, `examples/std_alias_pkg` |

For mixed-model workspaces, editor tests should also cover
`examples/mixed_models_workspace`.

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
