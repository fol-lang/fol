# Tool Commands

This chapter lists the current frontend surface by workflow area.

## Work

Project and workspace commands:

- `fol work init`
- `fol work new`
- `fol work info`
- `fol work list`
- `fol work deps`
- `fol work status`

Examples:

```text
fol work init --bin
fol work init --workspace
fol work new demo --lib
fol work info
fol work deps
```

Use `work` for:

- creating package/workspace roots
- inspecting workspace structure
- seeing member and dependency state

Scaffold reminder:

- `fol work init --bin` creates a hosted binary package with `fol_model = "memo"`
- the generated `build.fol` explicitly declares bundled `std` through
  `build.add_dep({ alias = "std", source = "internal", target = "standard" })`
- source code that uses bundled-library names imports that declared alias with
  `use std: pkg = {"std"};`

## Pack

Package acquisition commands:

- `fol pack fetch`
- `fol pack update`

Examples:

```text
fol pack fetch
fol pack fetch --locked
fol pack fetch --offline
fol pack fetch --refresh
fol pack update
```

Use `pack` for:

- materializing dependencies
- writing or honoring `fol.lock`
- refreshing pinned git dependencies

## Code

Build-oriented commands:

- `fol code check`
- `fol code build`
- `fol code run`
- `fol code test`
- `fol code emit rust`
- `fol code emit lowered`
- `fol code explain <CODE>`

Examples:

```text
fol code check
fol code build --release
fol code run -- --flag value
fol code emit rust
fol code emit lowered
fol code explain T1003
```

Use `code` for:

- driving the compile pipeline
- building binaries through the current Rust backend
- running produced binaries
- emitting backend/debug artifacts
- explaining a diagnostic code emitted by `fol code check`

### Explain

`fol code explain <CODE>` prints an extended, plain-language explanation for a
diagnostic code — the same code the pretty diagnostic footer points at
(`run \`fol code explain T1003\` for more`). It pairs with `fol code check`,
which emits the diagnostics it explains.

- `fol code explain <CODE>`

Codes are accepted case-insensitively (`t1003` and `T1003` are the same).

Examples:

```text
fol code explain T1003
fol code explain t1003
fol code explain --output json R1003
```

Output modes:

- `human` (default): a family chip, the code, a short title, and the body
- `plain`: `code:` / `family:` / `title:` lines followed by the body
- `json` / `--json`: `{ "code", "family", "known", "title", "explanation" }`

Behavior:

- known codes print their explanation and exit `0`
- unknown codes print an honest "no extended explanation for `<CODE>` yet"
  message (pointing at the code's family when the prefix is recognized) and
  exit nonzero

Only diagnostic codes the compiler and runtime actually emit have
explanations. The registry lives in `fol-diagnostics` (compiler truth) and is
kept honest by a completeness test, so `explain` never documents a code that
does not exist.

## Tool

Tooling commands:

- `fol tool lsp`
- `fol tool format <PATH>`
- `fol tool parse <PATH>`
- `fol tool highlight <PATH>`
- `fol tool symbols <PATH>`
- `fol tool references <PATH> --line <LINE> --character <CHARACTER>`
- `fol tool rename <PATH> --line <LINE> --character <CHARACTER> <NEW_NAME>`
- `fol tool semantic-tokens <PATH>`
- `fol tool tree generate <PATH>`
- `fol tool clean`
- `fol tool completion`

Examples:

```text
fol tool parse src/main.fol
fol tool format src/main.fol
fol tool highlight src/main.fol
fol tool symbols src/main.fol
fol tool references src/main.fol --line 12 --character 8
fol tool rename src/main.fol --line 12 --character 8 total
fol tool semantic-tokens src/main.fol
fol tool tree generate /tmp/fol
fol tool lsp
fol tool completion bash
```

Use `tool` for:

- editor integration
- Tree-sitter debugging
- LSP serving
- generated tool assets

### Parse And Query Results

`parse`, `highlight`, and `symbols` execute the checked-in generated FOL
Tree-sitter parser in-process. They do not estimate results from source text or
report the contents of query files as if those were matches.

`fol tool parse <PATH>` reports:

- `parse_status=ok` for a tree with no `ERROR` or missing nodes, otherwise
  `parse_status=ERROR`
- root kind plus total and named node counts
- exact `error_count` and `missing_count` values
- one zero-based source range and escaped source excerpt for every error or
  missing node
- the real Tree-sitter S-expression as `syntax_tree=...`

The command is error-tolerant: a source file with invalid syntax still produces
its recovered tree and exits through the normal command-result path. For
example, the removed `select(channel as value) { ... }` form reports an
`ERROR` node rather than being accepted as a second select grammar.

`fol tool highlight <PATH>` runs `queries/fol/highlights.scm` against that tree
and reports the actual capture count, capture kinds, and every capture as:

```text
capture=<name>@<start-row>:<start-column>-<end-row>:<end-column>:<text>
```

`fol tool symbols <PATH>` runs `queries/fol/symbols.scm`, reports the actual
symbol and scope counts, and reports each non-scope capture in the equivalent
`symbol=<name>@...:<text>` form. Rows and columns are zero-based. Backslashes,
tabs, carriage returns, and newlines in excerpts are escaped.

These three normal commands need no external `tree-sitter` executable. The
external CLI is required only by `fol tool tree generate`, which regenerates an
exportable parser bundle.

The public editor surface stays under `fol tool ...`.
There is no parallel `fol editor ...` command group.
Future editor features are not exposed as placeholder commands.
Only the shipped `fol tool` subcommands above are public.

## Artifact Reporting

Frontend commands report explicit artifact roots when applicable, including:

- emitted Rust crate roots
- lowered snapshot roots
- final binary paths
- fetch/store/cache roots where relevant
