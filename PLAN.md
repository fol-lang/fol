# Production Readiness Plan

Last updated: 2026-03-28

Current position: V2 Milestone 2 landed.

This document is the audit-driven work plan for making the FOL compiler,
runtime, editor, and tooling production-ready. Every item is a concrete finding
from a full codebase audit, not a speculative wish.

Items are grouped by severity and area. Each item includes the exact file, line,
and what is wrong.

This plan does NOT flag:

- features that are explicitly marked as future/unsupported with proper errors
- V3/V4 work that hasn't started (that's expected)
- parser nodes for future syntax that are rejected at typecheck (that's the
  intended design)

This plan DOES flag:

- silent wrong behavior (compiles but does the wrong thing)
- inconsistencies between pipeline stages within V1-V2M2 scope
- editor/tree-sitter drift from the actual compiler
- infrastructure issues that affect release quality

---

## Phase 1: Silent Wrong Behavior

These are the highest priority. The compiler produces output that is silently
incorrect or silently incomplete.

### 1.1 Entry point placeholder emits broken binary (complete, verified 2026-04-10)

File: `lang/execution/fol-backend/src/emit/skeleton.rs`
Line: 47

When entry resolution fails, the backend emits:

```rust
None => "    let _entry_name = \"placeholder\";".to_string(),
```

instead of returning an error. The user gets a binary that compiles, links, and
runs — but calls nothing. There is no warning, no error, no diagnostic.

This happens when:

- the entry routine has a receiver type
- the entry routine has parameters
- the entry signature is missing
- the namespace layout can't be found

Fix: return a `BackendError` instead of emitting placeholder code. A binary
that doesn't call the user's entry point is worse than a failed build.

### 1.2 Build evaluator silently swallows unknown expressions (complete, verified 2026-04-10)

File: `lang/execution/fol-build/src/executor/eval_expr.rs`
Line: 200

The catch-all arm in expression evaluation:

```rust
_ => Ok(None),
```

Any `AstNode` variant that the evaluator doesn't handle is silently treated as
"no value." If a user writes a `build.fol` with a construct the evaluator
doesn't recognize, nothing fails. The build proceeds with missing data.

Fix: return an `Err(BuildEvaluationError)` with a message naming the
unrecognized node kind.

### 1.3 Build evaluator silently skips while-loops (complete, verified 2026-04-10)

File: `lang/execution/fol-build/src/executor/core.rs`
Lines: 314-318

```rust
LoopCondition::Condition(_) => {
    // While-like loops are not supported in build evaluation
}
```

The condition-based loop branch does nothing and returns `Ok(())`. If a
`build.fol` uses a while-like loop, its body is silently skipped.

Fix: return an `Err(BuildEvaluationError)` with a clear message that
condition-based loops are not supported in build evaluation.

### 1.4 Build condition evaluator returns false for unknown nodes (complete, verified 2026-04-10)

File: `lang/execution/fol-build/src/executor/eval_condition.rs`
Line: 79

Unknown condition AST nodes evaluate to `Ok(false)`. If a `when` clause in
`build.fol` uses an unrecognized pattern (Is/Has/In/On), the clause silently
evaluates to false.

Fix: return an error instead of a default false.

### 1.5 Mutable global initialization uses Rust Default::default() (complete, verified 2026-04-10)

File: `lang/execution/fol-backend/src/instructions/render.rs`
Lines: 77-79

Mutable globals are initialized with:

```rust
*{global_path}.get_or_init(|| std::sync::Mutex::new(Default::default()))
    .lock().unwrap_or_else(|e| e.into_inner()) = {value}.clone();
```

The `Default::default()` call uses Rust's `Default` trait, not the FOL type's
declared default. For most types this is harmless because the value is
immediately overwritten by the store instruction. But if the global is read
before being explicitly written (possible in multi-function programs), the
value is Rust-default, not FOL-default.

Fix: either validate that globals are always written before read at the
lowering stage, or emit FOL-correct default values in the initializer.

---

## Phase 2: Typecheck-Lower Pipeline Gaps (V1 Scope)

These are features within the V1 language scope that pass typechecking but fail
at lowering. The user sees a confusing late error instead of a clear early
rejection.

### 2.1 Anonymous routines with captures (complete, verified 2026-04-10)

Typecheck: accepts
Lower: rejects at `lang/compiler/fol-lower/src/exprs/expressions.rs:1122-1126`

```
anonymous routines with captures are not yet supported
```

If captures are V1 scope, lowering should be implemented. If they're future
work, typecheck should reject them with a boundary message.

### 2.2 Complex type annotations in anonymous routines (complete, verified 2026-04-10)

Typecheck: accepts
Lower: rejects at `lang/compiler/fol-lower/src/exprs/expressions.rs:1090-1094`

```
complex type annotation in anonymous routine is not yet supported
```

Same decision needed: V1 or future. If future, reject in typecheck.

### 2.3 When expressions without default branch (complete, verified 2026-04-10)

Typecheck: accepts
Lower: rejects at `lang/compiler/fol-lower/src/exprs/flow.rs:544-546`

```
when expressions require a default branch
```

If this is a language rule, typecheck should enforce it. If lowering needs it
as a technical requirement, it should still be caught before lowering.

### 2.4 Type-matching when/of branches (complete, verified 2026-04-10)

Typecheck: accepts
Lower: rejects at `lang/compiler/fol-lower/src/exprs/flow.rs:767-769`

This is the one explicitly defined `UnsupportedLoweringSurface` in
`lang/compiler/fol-lower/src/boundaries.rs`:

```rust
pub enum UnsupportedLoweringSurface {
    TypeMatchingWhenOf,
}
```

Explicitly marked as V1 boundary. This should either be implemented or
rejected earlier.

### 2.5 Specific unary/binary operator lowering gaps (complete, verified 2026-04-10)

Typecheck: accepts various operators
Lower: rejects at `lang/compiler/fol-lower/src/exprs/expressions.rs`

- Line 141-145: specific unary operators produce dynamic error messages
- Line 181-186: specific binary operators produce dynamic error messages

These are V1 operators that typecheck passes. Either implement lowering or
reject at typecheck.

### 2.6 Loop expression lowering (complete, verified 2026-04-10)

Typecheck: accepts
Lower: rejects at `lang/compiler/fol-lower/src/exprs/expressions.rs:885-887`

```
loop lowering is not yet implemented
```

### 2.7 Block expression lowering (complete, verified 2026-04-10)

Typecheck: accepts
Lower: rejects at `lang/compiler/fol-lower/src/exprs/expressions.rs:889-891`

```
block expression lowering is not yet implemented
```

### 2.8 Template call lowering (complete, verified 2026-04-10)

Typecheck: accepts
Lower: rejects at `lang/compiler/fol-lower/src/exprs/expressions.rs:773-775`

```
template call lowering is not yet implemented
```

### 2.9 Procedure-style method calls as expression values (complete, verified 2026-04-10)

Typecheck: accepts
Lower: rejects at `lang/compiler/fol-lower/src/exprs/expressions.rs:340-342`

```
procedure-style method call '{method}' cannot be used as an expression value
```

This should be a typecheck error, not a lowering error.

### 2.10 Procedure-style calls without value result (complete, verified 2026-04-10)

Lower: rejects at `lang/compiler/fol-lower/src/exprs/calls.rs:823-824`

```
procedure-style calls without a value result are not lowered in this slice yet
```

---

## Phase 3: Typecheck-Lower Pipeline Gaps (V2M1/V2M2 Boundary)

These are correctly rejected at the right stage with explicit boundary
messages. They are listed here for completeness and to confirm they are
intentional, not accidental.

No action needed unless the boundary is wrong.

### V2M1 Boundaries (Generics)

- Generic routine lowering: `fol-lower/src/decls/routine_decls.rs:14-16`
- Generic receiver types: `fol-typecheck/src/decls.rs:556`
- Generic error types: `fol-typecheck/src/decls.rs:562`
- Generic routine constraints: `fol-typecheck/src/decls.rs:1670`
- Generic parameter type translation: `fol-lower/src/session.rs:223-227`

### V2M2 Boundaries (Standards)

- Blueprint standards: `fol-typecheck/src/decls.rs:235`
- Extended standards: `fol-typecheck/src/decls.rs:238`
- Generic standard routine requirements: `fol-typecheck/src/decls.rs:639`
- Receiver-qualified standard requirements: `fol-typecheck/src/decls.rs:644`
- Capturing standard routine requirements: `fol-typecheck/src/decls.rs:649`
- Default standard routine implementations: `fol-typecheck/src/decls.rs:654`
- Non-signature protocol members: `fol-typecheck/src/decls.rs:704`
- Protocol standard lowering: `fol-lower/src/session.rs:155-159`

---

## Phase 4: Tree-Sitter Grammar Sync

The tree-sitter grammar is approximately 35-40% complete relative to the
actual parser. This means editor syntax highlighting, code navigation, and
scope tracking break on valid FOL code.

### 4.1 Missing top-level declarations (9 types) (complete, verified 2026-04-10)

File: `lang/tooling/fol-editor/tree-sitter/grammar.js`

Grammar has 7 declaration types: `use`, `var`, `fun`, `log`, `typ`, `ali`,
plus `comment`/`doc_comment`.

Parser handles 16+ (dispatch at
`lang/compiler/fol-parser/src/ast/parser_parts/program_parsing.rs:167-510`):

| Missing from Grammar | Parser Line |
|---|---|
| `let` (binding) | 213 |
| `con` (constant) | 236 |
| `lab` (label) | 259 |
| `def` (definition) | 374 |
| `seg` (segment) | 305 |
| `imp` (implementation) | 328 |
| `std` (standard) | 351 |
| `pro` (procedure) | 489 |
| binding alternatives | 167 |

### 4.2 Missing control flow statements (8 types) (complete, verified 2026-04-10)

Grammar has: `return`, `break`, `when`, `loop`, `panic`, `report`,
`unreachable`, `check`.

Missing:

| Statement | Parser Reference |
|---|---|
| `if`/`else` | program_parsing.rs:724 |
| `select` | program_parsing.rs:733 |
| `yield` | program_parsing.rs:697 |
| `defer` | program_parsing.rs:706 |
| `while` | loop parsing:987 |
| `for` | loop parsing:989 |
| `each` | loop parsing:991 |
| `assert` | program_parsing.rs:603 |

### 4.3 Missing keyword operators (13+ operators) (complete, verified 2026-04-10)

Grammar defines symbol-based operators only (`==`, `!=`, `<`, `>`, `+`, `-`,
`*`, `/`, `%`, `&&`, `||`).

Missing keyword operators from
`lang/compiler/fol-lexer/src/token/buildin/mod.rs`:

- Logic: `or`, `xor`, `nor`, `and`, `nand`, `not`
- Comparison: `as`, `cast`, `is`, `has`, `in`, `on`, `of`, `at`
- Power: `^`
- Range: `..`, `...`

### 4.4 Missing expression forms (complete, verified 2026-04-10)

- Range expressions (`start..end`, `..end`, `start..`)
- If/else as expressions
- Select expressions
- Anonymous functions/closures
- Power expressions (`a ^ b`)

### 4.5 Query file coverage (complete, verified 2026-04-10)

The `.scm` files under `lang/tooling/fol-editor/queries/fol/` only reference
grammar rules that exist. Once the grammar is updated, the following query
files need updates:

- `highlights.scm` — add highlighting for all new declaration types, control
  flow, keyword operators
- `symbols.scm` — add symbol types for segments, standards, labels, constants
- `locals.scm` — add scope tracking for `let`, `con`, `lab` bindings

### 4.6 Keyword count summary (complete, verified 2026-04-10)

Lexer defines 43+ keywords. Grammar references 14. Gap: 29 keywords are
treated as identifiers by tree-sitter.

---

## Phase 5: Editor/LSP Gaps

These are features the LSP advertises in its capability registration
(`lang/tooling/fol-editor/src/lsp/mod.rs:64-111`) but only partially
implements.

### 5.1 Document symbols have no hierarchy (complete, verified 2026-04-10)

File: `lang/tooling/fol-editor/src/lsp/semantic.rs`
Line: 1293

```rust
children: Vec::new(),
```

Every symbol is flat. The outline view in any editor shows a flat list with no
nesting by scope, namespace, or type. Records don't contain their fields,
modules don't contain their declarations.

Fix: walk the resolved workspace tree and populate children for container
declarations (records, entries, modules, namespaces).

### 5.2 Workspace symbols only search open documents (complete, verified 2026-04-10)

File: `lang/tooling/fol-editor/src/lsp/mod.rs`
Lines: 478-490

The workspace symbol handler iterates `self.session.documents` which is only
the set of currently open files. If a user searches for a symbol that exists in
a file they haven't opened, it won't appear.

Fix: on workspace symbol requests, trigger analysis of all `.fol` files in the
workspace root, not just open documents.

### 5.3 Completion degrades to regex fallbacks (complete, verified 2026-04-10)

File: `lang/tooling/fol-editor/src/lsp/semantic.rs`

Multiple `fallback_*` functions (lines 732, 789, 797) use text pattern
matching when the compiler-backed semantic analysis doesn't produce results.
These are marked `// FALLBACK:` in the source.

Functions:

- `fallback_local_named_type_items` (line 732)
- `fallback_qualified_completion_items` (line 789)
- `fallback_imported_package_items` (line 797)

These produce plausible but semantically incorrect completions. For production,
either make the compiler-backed path work for these cases or clearly mark
fallback items as uncertain.

### 5.4 Code actions limited to structured suggestions only (complete, verified 2026-04-10)

File: `lang/tooling/fol-editor/src/lsp/semantic.rs`
Lines: 144-147

Code actions are only returned for diagnostics that have both
`suggestion.replacement` and `suggestion.location` present. Most compiler
diagnostics don't include structured suggestions, so the code action provider
returns empty for most errors.

Fix: add structured suggestions to the most common compiler diagnostics
(type mismatches, missing imports, name typos).

### 5.5 Rename restricted to same-file and current-package (complete, verified 2026-04-10)

File: `lang/tooling/fol-editor/src/lsp/semantic.rs`
Lines: 1148-1150, 1183-1186

Cross-package symbols are explicitly rejected:

```
rename currently supports same-file local and current-package top-level
symbols only
```

And:

```
rename currently refuses multi-package symbols
```

This is documented as a limitation in CLAUDE.md. Not a bug, but worth noting
for production readiness.

---

## Phase 6: Backend Code Quality

### 6.1 Routine shells emit todo!() (complete, verified 2026-04-10)

File: `lang/execution/fol-backend/src/signatures.rs`
Lines: 105, 107

```rust
format!("{header} {{\n    todo!()\n}}\n")
```

The `render_routine_shell()` function emits Rust `todo!()` in function bodies.
This is tested (line 528 asserts `contains("todo!()")`). Need to verify this
path is never reached during normal compilation. If it is, the compiled binary
panics at runtime with a `todo!()` instead of a meaningful FOL error.

Audit: trace all callers of `render_routine_shell()` and confirm none are on
the normal compilation path.

### 6.2 StdDecl nodes silently produce no type (complete)

File: `lang/compiler/fol-typecheck/src/exprs/mod.rs`
Line: 631

```rust
AstNode::StdDecl { .. } => Ok(TypedExpr::none()),
```

Standard declarations in the type checker produce no typed expression and no
validation. This is intentional for build.fol files, but should be verified
that StdDecl nodes never appear in non-build contexts.

---

## Phase 7: Infrastructure

### 7.1 Release toolchain pinned at Rust 1.70.0 (complete, verified 2026-04-10)

File: `.github/workflows/release.yml`

The release CI uses `rust-build.action@v1.4.3` with toolchain `1.70.0` (June
2023). Development and test CI use current Rust (1.93.0+). This version gap
means:

- release binaries may fail to compile if any dependency requires newer Rust
- language features used in the codebase may not be available in 1.70.0
- the release binary may behave differently from the tested binary

Fix: update release toolchain to match test CI, or at minimum to the current
MSRV (minimum supported Rust version) of the workspace.

### 7.2 Orphaned book page (complete, verified 2026-04-10)

File: `book/src/055_build/900_direction.md`

This file exists but is not referenced in `book/src/SUMMARY.md`. It won't
appear in the built book. Either add it to SUMMARY.md or delete it.

### 7.3 Missing authors field in 7 crate Cargo.tomls (complete, verified 2026-04-10)

These crates are missing the `authors` field (inconsistent with others):

- `lang/compiler/fol-intrinsics/Cargo.toml`
- `lang/compiler/fol-typecheck/Cargo.toml`
- `lang/compiler/fol-lower/Cargo.toml`
- `lang/execution/fol-backend/Cargo.toml`
- `lang/execution/fol-runtime/Cargo.toml`
- `lang/tooling/fol-editor/Cargo.toml`
- `lang/tooling/fol-frontend/Cargo.toml`

Minor, but affects `cargo metadata` consistency and crate publishing.

---

## Phase 8: What Is Solid (No Action Needed)

These areas passed audit with no issues.

### Compiler

- **fol-stream**: complete, no stubs, proper file/location tracking
- **fol-lexer**: complete, clean token/error model
- **fol-parser**: ~95% complete, all unsupported patterns return proper errors
- **fol-diagnostics**: complete, severity model, cascade suppression, hard cap
- **fol-resolver**: well-implemented, 12 unsupported features all return proper
  `ResolverErrorKind::Unsupported` errors
- **fol-intrinsics**: clean registry with explicit status/availability/lowering
  fields per entry, 58 deferred intrinsics are by design

### Runtime

- **fol-runtime**: production-quality, no stubs anywhere
  - core/memo/std split enforced by source assertions in tests
  - FolOption, FolError, FolRecover all fully implemented
  - containers use deterministic ordering (BTreeSet/BTreeMap)
  - bounds checking with RuntimeError
  - echo() is a real println!, not a stub

### Test Suite

- no `#[ignore]`d tests
- 14 `fail_*` example packages as intentional negative tests
- boundary tests verify proper rejection messages for V2M1, V2M2, and future
  features
- end-to-end integration tests compile and execute working examples
- tree-sitter sync tests verify highlighting stays consistent
- language-fact invariant tests verify type/keyword uniqueness

### V2M2 Standards Coverage

- protocol standard declarations tested
- conformance checking tested (18+ test cases in
  `test/typecheck/test_typecheck_standards_m2.rs`)
- blueprint/extended rejection tested
- generic standard requirement rejection tested
- 3 working example packages: `standards_protocol_m2`,
  `standards_protocol_pair_m2`, `standards_protocol_multi_m2`
- 5 failing example packages testing boundary enforcement

---

## Recommended Execution Order

1. **Phase 1** (silent wrong behavior) — highest priority, fixes correctness
2. **Phase 2** (typecheck-lower V1 gaps) — reduces confusing late errors
3. **Phase 4** (tree-sitter sync) — fixes daily editor experience
4. **Phase 7.1** (release toolchain) — prevents release pipeline breakage
5. **Phase 5** (LSP gaps) — improves editor quality
6. **Phase 6** (backend audit) — validates no runtime panics from todo!()
7. **Phase 7.2-7.3** (minor infrastructure) — cleanup

Phase 3 items are informational only — they are correctly handled V2 boundary
enforcement, not bugs.
