# Editor Tooling

FOL editor support lives in one crate:

- `fol-editor`

That crate owns two related subsystems:

- Tree-sitter assets for syntax-oriented editor work
- the language server for compiler-backed editor services

## Public Entry

The public entrypoints are exposed through `fol tool`:

- `fol tool lsp`
- `fol tool format <PATH>`
- `fol tool parse <PATH>`
- `fol tool highlight <PATH>`
- `fol tool symbols <PATH>`
- `fol tool references <PATH> --line <LINE> --character <CHARACTER>`
- `fol tool rename <PATH> --line <LINE> --character <CHARACTER> <NEW_NAME>`
- `fol tool semantic-tokens <PATH>`
- `fol tool tree generate <PATH>`

This keeps editor workflows under the same `fol` binary rather than introducing
a second public tool.

## Split Of Responsibilities

Tree-sitter is the editor syntax layer.

It is responsible for:

- syntax trees while typing
- query-driven highlighting
- locals and symbol-style structure captures
- editor-facing structural parsing

The language server is the semantic editor layer.

It is responsible for:

- JSON-RPC/LSP transport
- open-document state
- compiler-backed diagnostics
- hover
- go-to-definition
- whole-document formatting
- code actions for exact compiler suggestions
  The current shipped inventory is intentionally one diagnostic family:
  unresolved-name replacements where the compiler attached an exact replacement.
- signature help for plain and qualified routine calls
- references
- rename for same-file local and current-package top-level symbols
- semantic tokens
- document symbols
- workspace symbols across discovered `.fol` files under the mapped workspace root
- completion

The currently supported v1 LSP surface is:

- diagnostics
- hover
- definition
- formatting
- code actions for exact compiler suggestions
- signature help for plain and qualified routine calls
- references
- rename for same-file local and current-package top-level symbols
- semantic tokens
- document symbols
- workspace symbols across discovered `.fol` files under the mapped workspace root
- completion

The server keeps diagnostics and semantic snapshots separately.

Formatting is intentionally whole-document only right now.
`textDocument/rangeFormatting` stays unsupported until there is a safe
structure-preserving boundary instead of a partial line-rewriter.

The current formatter contract is intentionally narrow but explicit:

- indentation is four spaces per brace depth
- lines are trimmed before indentation is re-applied
- leading blank lines are removed and repeated blank lines collapse to one
- trailing blank lines are removed
- output always ends with one final newline when the document is non-empty
- line endings are normalized to `\n`
- `build.fol` follows the same formatter entrypoint and indentation rules as
  ordinary source files

Diagnostics refresh on `didOpen` and `didChange`.

Semantic requests keep one semantic snapshot per open document version and
reuse it for hover, definition, signature help, references, rename, semantic
tokens, document symbols, workspace symbols, and completion until the document
changes or closes.

## Compiler Truth

`fol-editor` does not create a second semantic engine.

Semantic truth still comes from:

- `fol-package`
- `fol-resolver`
- `fol-typecheck`
- `fol-diagnostics`

So the model is:

- Tree-sitter answers “what does this text structurally look like?”
- compiler crates answer “what does this code mean?”

For the current maintenance contract between compiler crates, LSP behavior, and
Tree-sitter assets, see:

- [Compiler Integration](./350_compiler_integration.md)
- [Feature Update Checklist](./450_feature_checklist.md)

## Current Practical Workflow

Use:

```text
fol tool lsp
fol tool format path/to/file.fol
```

as the language server entrypoint.

Launch it from inside a discovered package or workspace root. The frontend
looks upward for `build.fol` or `fol.work.yaml` before starting the server.

Use:

```text
fol tool parse path/to/file.fol
fol tool highlight path/to/file.fol
fol tool symbols path/to/file.fol
fol tool format path/to/file.fol
fol tool references path/to/file.fol --line 12 --character 8
fol tool rename path/to/file.fol --line 12 --character 8 total
fol tool semantic-tokens path/to/file.fol
```

for parser/query debugging and validation.
