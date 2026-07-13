# Compiler Integration

This chapter explains how the compiler and editor tooling are connected today,
and what should remain the source of truth as FOL grows.

## The Three Layers

FOL editor tooling is split into three layers:

1. compiler truth
2. semantic editor services
3. syntax editor services

Compiler truth lives in crates such as:

- `fol-lexer`
- `fol-parser`
- `fol-package`
- `fol-resolver`
- `fol-typecheck`
- `fol-intrinsics`
- `fol-diagnostics`

Semantic editor services live in:

- `fol-editor` LSP analysis and semantic code

Syntax editor services live in:

- the Tree-sitter grammar
- Tree-sitter query files
- editor bundle generation

That split matters because not every editor feature should be solved in the
same way.

## What The LSP Should Trust

For semantic meaning, the LSP should trust the compiler pipeline.

That means:

- diagnostics come from compiler diagnostics
- hover should be derived from resolved or typed compiler state
- definition should be derived from resolved symbol data
- document symbols should prefer compiler symbol ownership where practical
- completion should prefer compiler facts over text heuristics

The LSP should not invent a parallel semantic model.

If a feature needs semantic meaning, the first question should be:

- can this come from `fol-parser` / `fol-resolver` / `fol-typecheck` instead of editor-only code?

## What Tree-sitter Should Trust

Tree-sitter is not the compiler parser.

It exists for:

- syntax trees while typing
- highlighting
- locals queries
- symbol-style structural captures
- future textobjects and editor movement

So Tree-sitter should stay editor-oriented.

That means:

- the grammar can remain handwritten
- query files can remain handwritten
- but duplicated language facts should not be copied manually forever

Examples of facts that should come from compiler-owned truth:

- builtin type spellings
- implemented intrinsic names
- import source kinds
- keyword families used by syntax tooling

## How Diagnostics Flow

Today the intended direction is:

1. compiler stage creates a structured `fol_diagnostics::Diagnostic`
2. editor tooling adapts that diagnostic for the active editor protocol
3. the editor displays the adapted result

The important rule is:

- the compiler diagnostic object is canonical

So when diagnostic wording, labels, helps, or codes change, the editor should
adapt that same structure. It should not create its own free-form diagnosis
logic.

## What Can Be Generated

Generation is useful when the same language fact appears in multiple places.

Good generation targets:

- keyword manifests
- builtin type manifests
- intrinsic name/surface manifests
- source-kind manifests
- small generated query fragments or validation snapshots

Bad generation targets:

- the entire Tree-sitter grammar
- Tree-sitter precedence/conflict structure
- most highlight policy
- LSP transport behavior

The rule of thumb is:

- generate shared facts
- handwrite editor behavior

## What To Update For A New Feature

When you add a new language feature, ask these questions in order:

1. Does the lexer need new tokens or token families?
2. Does the parser need new syntax or AST nodes?
3. Does resolver or typechecker logic need new meaning?
4. Do lowering, runtime, or backend transfer/execution rules change?
5. Does frontend artifact routing or capability evaluation change?
6. Does the feature introduce or change diagnostics and explanations?
7. Does the formatter or a public `fol tool` inspection command need an update?
8. Does the LSP need hover, completion, definition, symbols, tokens, positions,
   or artifact-scope updates?
9. Does Tree-sitter need grammar, query, or executable corpus updates?
10. Can any new editor-visible facts be generated from compiler-owned sources?
11. Do canonical positive/failure inventories, docs, or book claims change?

If the answer to any of those is yes, update that layer in the same change set.

## End-To-End Feature Completeness

Compiler-backed editor analysis prevents semantic duplication, but it does not
make the other mirrors automatic. A shipped feature is complete only when its
meaning survives:

- lowering and runtime/backend execution
- frontend artifact selection and capability routing
- structured CLI diagnostics, explanations, and LSP adaptation
- whole-document formatting and public parse/highlight/symbol commands
- explicit LSP UX such as completion, tokens, navigation, and position mapping
- Tree-sitter grammar, highlight/locals/symbol queries, and executable corpus
- canonical positive/failure examples, integration tests, docs, and the book

For V3 specifically, this rule covers every ownership, borrow, pointer, `dfr` /
`edf`, spawn, channel, `select`, `[mux]`, async, and await boundary. Reusing the
typechecker is necessary, but it is not permission to skip an explicit syntax,
tooling, inventory, or documentation audit.

## Current Practical Rule

In FOL today:

- compiler crates own meaning
- `fol-editor` owns editor protocol behavior
- Tree-sitter owns syntax-oriented editor structure

The safer future direction is:

- move shared contracts closer to compiler-owned crates
- keep protocol/UI code in tooling crates
- generate duplicated facts instead of hand-copying them
