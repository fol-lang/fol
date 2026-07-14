# Tree-sitter Integration

The Tree-sitter side of FOL is the editor-facing syntax layer.

It is not the compiler parser.

## What Is In The Repo

The editor crate carries:

- the grammar source
- executable corpus cases with expected syntax trees
- a raw showcase fixture kept outside the corpus directory
- query files on disk

Canonical query assets live as real files, not just embedded Rust strings:

- `queries/fol/highlights.scm`
- `queries/fol/locals.scm`
- `queries/fol/symbols.scm`

This is intentional because editors such as Neovim expect query files on disk in
the standard Tree-sitter layout.

The checked-in generated C parser is also compiled into `fol-editor`. Therefore
`fol tool parse`, `fol tool highlight`, and `fol tool symbols` use the same
parser and query assets directly in-process. Their reported trees, captures,
symbols, scopes, and `ERROR` nodes are runtime results, not grammar/query byte
counts or source-text approximations. These inspection commands do not require
an external Tree-sitter installation.

## Generated Bundle

To generate a Neovim-consumable bundle, run:

```text
fol tool tree generate /tmp/fol
```

That writes a bundle containing the grammar, query assets, executable corpus
cases, and `test/fixtures/showcase.fol` under the target directory. The
showcase is intentionally a raw `.fol` fixture rather than a corpus case.
Generated file ownership is recorded in `.fol-tree-generated`: later runs
remove retired manifest-owned assets while preserving files the user added to
the destination. Unsafe absolute or parent-traversing manifest entries are
rejected rather than removed. The generator also rejects a symlink at the
destination root or anywhere along a managed asset path, so generated writes
and retired-file cleanup cannot escape the selected bundle root.

Generation and parser validation happen in a temporary staging directory.
Only a complete bundle is committed to the destination; a generator or commit
failure leaves an existing bundle unchanged, including its ownership manifest
and retired assets.

Bundle generation is the one public Tree-sitter command that invokes the
external `tree-sitter` CLI. The checked-in parser used by ordinary inspection
commands remains available without that executable.

The intended consumer path is:

- generate bundle
- point the editor's Tree-sitter parser configuration at that bundle
- let the editor compile/use the parser from there

## File Ownership

### Intentionally Handwritten

These files are human-authored and should remain so:

| File | Purpose | Owner |
|------|---------|-------|
| `tree-sitter/grammar.js` | Grammar rules, precedence, conflicts | Editor/syntax maintainer |
| `queries/fol/highlights.scm` | Highlight capture groups and query patterns | Editor/syntax maintainer |
| `queries/fol/locals.scm` | Scope and definition tracking | Editor/syntax maintainer |
| `queries/fol/symbols.scm` | Symbol navigation captures | Editor/syntax maintainer |
| `test/corpus/*.txt` | Corpus programs plus their expected node trees | Editor/syntax maintainer |

Do not attempt to generate these from the compiler parser. The parsing models
are different and auto-generation creates more fragility than value.

### Validated Against Compiler Constants

These facts appear in handwritten files but are validated by integration tests
to stay in sync with compiler-owned constants:

| Fact | Handwritten Location | Compiler Source |
|------|---------------------|-----------------|
| Builtin type names | `highlights.scm` regex `^(int\|bol\|...)$` | `BuiltinType::ALL_NAMES` in `fol-typecheck` |
| Dot-call intrinsic names | `highlights.scm` regex `^(len\|echo\|...)$` | Implemented `DotRootCall` entries in `fol-intrinsics` |
| Container type names | `highlights.scm` node labels + `grammar.js` choice | `CONTAINER_TYPE_NAMES` in `fol-parser` |
| Shell type names | `highlights.scm` node labels + `grammar.js` choice | `SHELL_TYPE_NAMES` in `fol-parser` |
| Source kind names | `highlights.scm` node labels + `grammar.js` choice | `SOURCE_KIND_NAMES` in `fol-parser` |

The sync tests live in `test/run_tests.rs` under `treesitter_sync`. If you add
a new builtin type, intrinsic, container, shell, or source kind to the compiler,
these tests fail until the tree-sitter files are updated to match.

## When To Update Tree-sitter Files

### grammar.js

Update when:

- A new declaration form is added (e.g. a new `seg` or `lab` declaration)
- A new expression or statement form is added
- A new type syntax is added (e.g. a new container or shell family)
- A new source kind is added
- Operator precedence or conflict rules change

Do not update for:

- New diagnostic codes or error messages
- New resolver/typecheck rules that don't change syntax
- New intrinsics (these use existing `dot_intrinsic` grammar rule)

### highlights.scm

Update when:

- A new keyword needs highlighting
- A new builtin type is added to the compiler
- A new implemented dot-call intrinsic is added
- A new container or shell type family is added
- A new source kind is added
- Highlight group policy changes (e.g. moving a keyword from `@keyword` to `@keyword.function`)

### locals.scm

Update when:

- Scope rules change (e.g. new block forms that introduce scopes)
- Definition capture patterns change

### symbols.scm

Update when:

- New declaration forms should appear in document symbol navigation

### Corpus fixtures

Update when:

- Any grammar rule is added or modified
- A new syntax family needs parse-tree validation

## Corpus Coverage Expectations

Corpus fixtures live in `tree-sitter/test/corpus/` and cover syntax families:

| Corpus File | Covers |
|-------------|--------|
| `declarations.txt` | `use`, `ali`, `typ`, `fun`, `log`, `var` declarations |
| `expressions.txt` | Intrinsic calls, `when`/`loop` control flow, `break`/`return` |
| `recoverable.txt` | Error propagation (`/`), `report`, pipe-or (`\|\|`) |
| `v3_ownership.txt` | owned allocation, borrow options, `~var`, and give-back |
| `v3_pointers.txt` | nested `ptr[...]` / `chn[...]` types, `@` types, address-of, and dereference |
| `v3_deferred.txt` | `dfr` and error-only `edf` blocks |
| `v3_channels_select_mutex.txt` | spawn, channel endpoints, multi-arm `select`, and `[mux]` parameters |
| `v3_eventuals.txt` | spawn plus `\| async` and `\| await` stages |
| `v3_lexical_boundaries.txt` | multiline backtick/slash-block comments, exact `[doc]`, and raw single-quoted character/string boundaries |

Every file in this directory is a real Tree-sitter corpus case: the FOL source
appears before `---`, and the exact expected node tree appears after it. A raw
fixture without an expected tree belongs under `tree-sitter/test/fixtures/`,
not under `test/corpus/`.

Regenerate the checked-in bundle and execute the external corpus runner with:

```text
make tree-test
```

This lane fails when zero corpus cases execute, when any expected tree drifts,
or when any case fails. Ordinary Rust editor tests also export the bundle to a
temporary root and require the external runner to execute every registered
case successfully.

When a new syntax family is added, it should have corpus coverage. The expected
families that should each have at least one corpus example:

- Import declarations (`use`)
- Type declarations (`typ`, `ali`)
- Routine declarations (`fun`, `log`)
- Variable declarations (`var`)
- Control flow (`when`, `loop`, `case`, `break`, `return`)
- Expressions (binary, call, field access, dot intrinsic)
- Container and shell types
- Record and entry types
- Error handling (`report`, pipe-or, `check`)
- V3 ownership and pointer sigils in both expression and nested type positions
- V3 deferred blocks (`dfr` / `edf`)
- V3 processor syntax (`[>]`, `chn[...]`, channel endpoints, `select`, `[mux]`,
  `\| async`, and `\| await`)
- compiler-recognized lexical protection boundaries, especially multiline
  comments and raw single-quoted literals containing V3 sigils or braces

Tree-sitter keeps the compiler lexer's four comment classes visible: ordinary
backtick, exact `` `[doc]...` `` documentation comments, `//`, and non-nested
`/* ... */`. Backtick, documentation, and slash-block comments may span lines.
Single quotes remain raw: backslashes do not introduce escapes, one Unicode
scalar is a character, and empty or multi-scalar payloads are raw strings.

The removed single-header `select(channel as value) { ... }` and
double-parenthesis mutex-parameter forms are not alternate current syntax.
Their `fail_proc_*` examples guard rejection; if either spelling appears in a
tree-sitter corpus, it must be an explicit parse-error expectation rather than
a second accepted grammar path.

## What Tree-sitter Is For

Use the Tree-sitter layer for:

- highlighting
- locals and capture queries
- symbol-style structural views
- editor textobjects and movement later

Do not use it as a substitute for typechecking or resolution.

Those remain compiler tasks.

In particular, tree-sitter does not decide whether a unique-pointer field can
be dereferenced, whether a moved owner can be reinitialized in deferred work,
whether deferred cleanup may initiate `report`, whether a call is a legal
direct spawn/async target, whether a nested routine implicitly captures an
outer local, whether a recoverable eventual remains live across fallthrough or
an exiting `break`/`return`/`report`, whether `await` is used inside `edf`, or
whether mutex effects can occur inside `dfr`/`edf`. The LSP reports compiler
diagnostics for those semantic boundaries. Tree-sitter is responsible only for
the corresponding syntax shape, highlighting/query captures, and corpus
coverage. The exact positive and negative processor package matrix lives in the
[shipped processor inventory](../900_processor/_index.md#shipped-example-inventory).

When a language feature changes syntax, use the
[Feature Update Checklist](./450_feature_checklist.md) to decide whether the
grammar, queries, corpus, or generated language facts also need updates.
