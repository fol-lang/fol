# Feature Update Checklist

Use this checklist whenever a new language feature, syntax form, intrinsic, or
error surface is added.

This chapter is about maintenance discipline, not just implementation order.

## Quick Reference

When adding a feature, touch these layers in order:

1. **Lexer** — new keywords, operators, tokens
2. **Parser** — new AST nodes, syntax rules
3. **Semantics** — resolver, typecheck, lowering, intrinsics
4. **Runtime/backend** — representation, transfer, cleanup, execution
5. **Frontend/build routing** — artifact model, active dependencies, command legality
6. **Diagnostics** — codes, explanations, labels, related sites
7. **Formatter/tool commands** — protected text, parse/highlight/symbol output
8. **LSP** — hover, completion, definition, symbols, positions
9. **Tree-sitter** — grammar, queries, corpus
10. **Generated facts and inventories** — compiler-owned constants, exact examples
11. **Docs and book** — language chapters, tooling pages, boundary claims
12. **Tests and examples** — unit, integration, editor, corpus, positive/failure packages

Automated guards exist for some of these. The `treesitter_sync` integration
tests verify that `highlights.scm` matches compiler-owned constants for builtin
types, intrinsic names, container/shell types, and source kinds. Adding a new
constant to the compiler without updating Tree-sitter will fail those tests.

## 1. Lexical Surface

Check:

- new keywords
- new operators or punctuation
- new literal/token families
- comment or whitespace effects

Update:

- `fol-lexer`
- lexical docs under [`100_lexical`](../100_lexical/_index.md)
- any generated keyword/facts manifest if one exists

## 2. Parser Surface

Check:

- new declarations
- new expressions
- new statement forms
- new type forms
- new precedence or ambiguity rules

Update:

- `fol-parser`
- parser tests
- Tree-sitter grammar if the syntax is editor-visible
- Tree-sitter corpus fixtures for the new syntax family

## 3. Semantic Surface

Check:

- name resolution
- type checking
- lowering
- intrinsic availability
- runtime/backend impact

Update:

- `fol-resolver`
- `fol-typecheck`
- `fol-lower`
- `fol-intrinsics`
- runtime/backend crates if needed

## 4. Runtime And Backend Surface

Check:

- lowered representation and ownership-transfer mode
- runtime type or helper requirements
- cleanup/drop behavior on normal, error, branch, loop, and task paths
- capability-tier imports and backend emission
- executable behavior for every supported positive form

Update:

- `fol-lower`
- `fol-runtime`
- `fol-backend`
- emitted-code and end-to-end execution tests

## 5. Frontend And Build Routing

Check:

- artifact source scope and selected `fol_model`
- evaluated conditional dependencies and bundled-standard aliases
- direct, workspace, and named-step routes
- language API legality versus host target compatibility for `run` / `test`
- no accidental bundled-std requirement merely because an artifact executes
- ambiguous mixed-model files and packages

Update:

- `fol-build` / `fol-package` when graph truth changes
- `fol-frontend` routing and command behavior
- editor workspace mapping when analysis scope changes
- direct, routed, mixed-model, and conditional-build integration tests

Important rule:

- use the evaluated artifact contract as the authority
- do not reconstruct capability truth from static `build.fol` declarations
- do not fall back from an ambiguous mixed-model file to a wider model

## 6. Diagnostics Surface

Check:

- new error cases
- new warning/info cases
- changed wording for existing rules
- changed labels, notes, helps, or suggestions

Update:

- compiler producer diagnostics
- `fol-diagnostics` contract tests
- editor/LSP diagnostic adapter tests if the visible shape changes
- docs under [`650_errors`](../650_errors/_index.md) if behavior changed materially

## 7. Formatter And Tool Commands

Check:

- whether new syntax changes indentation or structural scanning
- whether comments, cooked strings, raw strings, or character literals may
  contain the new syntax without affecting formatting
- whether `fol tool parse`, `highlight`, or `symbols` expose the new shape
- whether command output, exit status, and parse-error recovery remain honest

Update:

- the shared compiler-aware source scanner before adding command-specific
  textual heuristics
- whole-document formatter tests
- in-process parser/query command tests
- public tool-command documentation

## 8. LSP Surface

Check:

- hover content
- go-to-definition behavior
- document symbols
- completion
- open-document analysis behavior under broken code
- semantic tokens, signature/type information, and rename eligibility
- UTF-16 position conversion for incoming and outgoing ranges
- related diagnostic URIs and multi-file locations
- active artifact scope and capability-aware suggestions

Update:

- `fol-editor` semantic analysis
- `fol-editor` semantic display helpers or compiler-owned helpers they consume
- LSP tests

Important rule:

- prefer compiler-backed meaning
- use fallback heuristics only when the compiler cannot supply semantic data yet

## 9. Tree-sitter Surface

Check:

- syntax shape visible while typing
- highlight groups
- locals captures
- symbols captures
- corpus examples

Update:

- `tree-sitter/grammar.js`
- `queries/fol/highlights.scm`
- `queries/fol/locals.scm`
- `queries/fol/symbols.scm`
- `tree-sitter/test/corpus/*.txt`

Important rule:

- Tree-sitter is for syntax-facing editor behavior
- it does not replace compiler semantics

## 10. Generated Facts And Inventories

Check whether the feature adds a new fact that should be exported once instead
of copied by hand.

Examples:

- intrinsic names
- builtin type names
- source kinds
- keyword groups
- shell/container family names
- positive and negative example package directories

If yes:

- update the compiler-owned source
- regenerate editor-facing artifacts from that source
- do not patch multiple copies manually
- update the canonical shared example inventory and its published book matrix
  together; do not create a private LSP-only list

## 11. Documentation

Update:

- the language chapter for the feature
- tooling docs if editor behavior changes
- diagnostics docs if compiler reporting changes
- examples/fixtures that demonstrate the preferred form

## 12. Tests And Examples

Add or update:

- compiler unit tests
- integration tests
- editor/LSP tests if semantic editor behavior changes
- Tree-sitter tests/corpus if syntax-facing behavior changes
- a runnable positive package for shipped behavior
- a `fail_*` package for each deliberate boundary or removed form
- the canonical inventory guard when an example directory is added, removed, or
  renamed

Keep the feature test in the same change as the feature.

## 13. Final Review Questions

Before considering the feature complete, answer:

1. Is compiler meaning implemented?
2. Does lowering/runtime/backend preserve that meaning on every control-flow path?
3. Does frontend routing select the exact evaluated artifact capability?
4. Are diagnostics coded, explained, structured, and location-correct?
5. Do formatter and public tool commands understand the new syntax boundaries?
6. Does the LSP reflect the meaning, model, ranges, and UX where needed?
7. Does Tree-sitter reflect the syntax in grammar, queries, and executable corpus?
8. Did we generate shared facts and reuse one canonical example inventory?
9. Do positive examples run and deliberate boundaries fail with the expected code?
10. Do docs and the book state the shipped behavior without preserving dead syntax?
