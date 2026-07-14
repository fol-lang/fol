# V3 Memory Pillar Plan

> **Status: complete.** This file is retained as the implementation record for
> the shipped V3 memory pillar. The current user-facing contract lives in the
> linked book chapters and the V3 section of `plan/VERSIONS.md`; present-tense
> planning language below describes the work as it was staged, not unfinished
> current behavior.

The V2 Deepening plan is complete. V3 is the systems-semantics release, and it
is split into two pillars that land in order:

- the **memory pillar** (this plan): ownership, borrowing, pointers, and
  ownership-aware `dfr`/`edf`
- the **processor pillar** (`plan/V3_PROC.md`): OS-thread tasks, channels,
  mutexes, and eventuals

This plan covers the memory pillar only. The theme is:

- ownership and lexical borrowing are **compile-time-only** resource discipline,
  legal in every runtime model, with no runtime ownership tags or borrow
  bookkeeping; `ptr[shared, T]` separately uses reference-count bookkeeping
- move/clone is a static rule decided at typecheck, not a runtime tag
- enforcement is **scope-granular**, not flow-sensitive — a scope-stack
  discipline in typecheck, never a dataflow/NLL solver
- heap allocation stays gated behind `memo`+, exactly like the existing `str` /
  `vec` heap gates
- `core` and `memo` artifacts remain executable without bundled `std`; model
  gates constrain source-language APIs, not frontend host process launching
- public `fol_model` has exactly the `core` and `memo` values; explicit
  `build.add_dep({ alias = "std", source = "internal", target = "standard" })`
  on a `memo` artifact supplies the separate hosted-library capability tier
- the current Rust backend may still use hosted implementation substrate;
  `core` is the no-heap FOL source/API contract, not a claim that the complete
  generated binary is already freestanding or allocation-free
- every feature change is mirrored through frontend capability routing,
  structured diagnostics and explanations, formatter/tool commands, the LSP,
  tree-sitter grammar/queries/corpus, examples, tests, docs, and the book in the
  **same** change set, never later
- the memory pillar completes fully (M1 -> M2 -> M3) **before** the processor
  pillar starts; the processor pillar consumes the move-at-boundary rule and the
  `name[options]: type` parameter grammar this plan introduces

The book chapters that this plan implements — and heavily rewrites — are:

- `book/src/800_memory/100_ownership.md`
- `book/src/800_memory/200_pointers.md`
- `book/src/700_sugar/250_dfr.md`

Every place this plan contradicts those chapters is enumerated in Workstream U.

The shipped transfer contract is more precise than the original shorthand
"stack clones, heap moves":

- clone-safe values clone on transfer
- heap-owned values, unique pointers, and aggregates containing unique
  ownership move on transfer
- borrowed values never transfer unique ownership; immutable borrows are
  read-only, while `var[mut, bor]` may update its owner without moving from it
- dereferencing a unique pointer clones a clone-safe pointee, but transfers a
  move-only pointee and consumes that pointer
- shared and borrowed pointer dereferences are read-only, so they require a
  clone-safe pointee

The shipped delayed-cleanup boundary is also explicit. `dfr` runs for every
exit from its lexical scope, including recoverable reports and panic, while
`edf` runs only for recoverable reports. Neither body may contain `return`,
`break`, `report`, or `panic`; delayed owner reinitialization, channel endpoint
acquisition, and mutex guard operations/forwarding are rejected.


# 1. Guardrails

Keep, permanently:

- ownership/borrow checking as a pure compile-time discipline
- scope-granular enforcement (borrows live to end of lexical scope)
- clone-on-transfer for clone-safe values and move-on-transfer for unique
  ownership, including aggregates that contain it
- `memo`+ gating for anything that allocates
- monomorphized, dispatch-free lowering
- the refined sigil charter (below), which preserves the book's sigil aesthetic

Never add, under any workstream in this plan:

- flow-sensitive / non-lexical borrow checking (NLL, dataflow liveness)
- a garbage collector of any kind
- destructors / RAII user hooks / user-defined drop bodies
- raw pointers, manual `.free()`, or any unsafe manual-delete surface (that is
  V4/FFI territory)
- weak references / cycle collection
- thread-safe sharing primitives inside the memory pillar (Arc/atomics belong to
  the processor pillar's mutex boundary, decided there)
- naming conventions that change semantics (ALL_CAPS-means-borrowable is deleted)
- any runtime tag, header word, or refcount used to decide move vs clone

If a workstream drifts toward these, it stops.

The sigil charter (fixed for all of V3):

- `@x`     — heap allocation, sugar for a `[new]` binding; legal in **type
             position** too, e.g. `opt @Node` means "optional owned heap `Node`"
- `!x`     — give-back **only**: early return of ownership to the original owner
- `#x`     — borrow-from expression sugar: produce a borrow of `x`
- `&x`     — address-of (pointer)
- `*p`     — by-value dereference (clone-safe read or consuming unique
             transfer), plus direct mutable unique-pointer write-through
- `[>]e`   — spawn (owned by the processor pillar; listed here only for the
             charter's completeness)
- `~var`   — accepted **sugar** for `var[mut]`; same AST, no new concept
- `var[~]` — **dead** spelling; must never parse

`ALL_CAPS`-parameter-means-borrowable is deleted. The `Parameter.is_borrowable`
field survives but is retriggered exclusively by the explicit
`name[bor]: T` parameter syntax introduced in Workstream S.


# 2. Pre-Implementation Truth Snapshot (Historical)

This was the verified baseline before the memory pillar landed. It is preserved
to explain the workstream decisions and must not be read as current compiler
behavior.

Parsed at that baseline, but semantically rejected:

- `ptr[T]` parses to `FolType::Pointer { target }` — **exactly one** type
  argument, and the AST node carries **no qualifier**
  (`lang/compiler/fol-parser/src/ast/types.rs:134`). Typecheck rejects it at
  `decls.rs:3288` ("pointer types are planned for a future release").
- `&x` parses as `UnaryOperator::Ref`, `*x` as `UnaryOperator::Deref`
  (`expression_atoms_and_literal_lowering.rs:347,354`). Typecheck rejects
  pointer operators at `exprs/operators.rs:345`.
- `@var name: T` prefix parses to `VarOption::New`
  (`binding_alternative_parsers.rs:15`); `var[new]` / `var[bor]` / `var[mut]`
  binding options parse (`binding_option_parsers.rs:38,39,31`). Typecheck rejects
  `[bor]` at `decls.rs:3466`, `[new]`/heap at `decls.rs:3471`, borrowable params
  at `decls.rs:3455`, mutex params at `decls.rs:3453`.
- `~var` prefix parsed to `VarOption::Mutable`; `var[~]` **also** parsed to
  Mutable at that baseline (`binding_option_parsers.rs:31`) — the charter kills
  the second spelling.
- `ALL_CAPS` parameter names set `Parameter.is_borrowable = true` at that
  baseline (`routine_header_parsers.rs:125` et al., `types.rs:252`).

Did **not** parse at that baseline — real grammar work was required:

- `@` in **type** position (`opt @Node`) — `special_type_parsers.rs` has no `@`
  type handler.
- `#x` borrow-from prefix sugar — `#`/`Hash` maps to no unary operator.
- `!x` give-back prefix — `!`/`Bang` was the `[!]` static-var sigil and
  the postfix `x!` unwrap (`UnaryOperator::Unwrap`,
  `postfix_expression_parsers.rs:162`); there is **no** give-back operator, and
  the prefix `!` slot collides with the static-var meaning.
- `ptr[shared, T]` / `ptr[raw, T]` — `ptr[...]` accepted exactly one argument at
  that baseline.
- `name[options]: type` parameter-option grammar (needed for `name[bor]:` and,
  in the processor pillar, `name[mux]:`) — parameters do not parse an option
  bracket list.

Recursive types were **rejected wholesale** at that baseline
(`reject_recursive_type_definition`, `fol-typecheck/src/decls.rs:2527`), because
the lowered layer is purely structural: `LoweredType`
(`fol-lower/src/types.rs:22`) has **no named/nominal variant** — `Record` inlines
its fields, so a self-referential record has no finite lowered shape. This is the
exact boundary M1's flagship replaces for the `@`-recursion case.

Lexer facts touched by prep:

- `defer` and `go` are both live keywords
  (`fol-lexer/src/token/buildin/mod.rs:11`); `go` is reserved but unused.

Diagnostics: `family_for_code` (`fol-diagnostics/src/explain.rs:31`) maps a
code's first byte to a family; `O` fell through to the generic
`ERROR` family. The registry-honesty test
`every_registered_code_has_a_recognized_family_prefix`
(`explain.rs`) fails the moment an `O####` code is registered without an `O`
family entry.

Model gating already exists: `TypecheckCapabilityModel` (Core/Memo/Std) drives
`fol-typecheck/src/model.rs`, and the backend emits per-tier runtime imports.


# 3. Exit Criteria

This plan is complete when all of the following are true:

1. Prep (Q) has landed: `defer` is spelled `dfr` everywhere, `go` is removed,
   the sigil charter is enforced, the `ALL_CAPS` borrowable hook is gone, and the
   `O####` OWNERSHIP diagnostic family exists end to end.
2. M1 (R), M2 (S), and M3 (T) are each implemented end to end — lexer/parser,
   resolver, typecheck, lowering, runtime/backend, frontend routing,
   diagnostics/explanations, formatter/tool commands, LSP, tree-sitter
   grammar/queries/corpus, positive and `fail_*` inventories, docs, and the book
   — and each landed workspace-green.
3. No "planned for a future release" rejection remains in the compiler for a
   memory surface this plan has chosen to build; each such site is either
   deleted or replaced with an honest permanent boundary message (raw pointers,
   weak refs).
4. The recursive-type rejection is replaced for `@`-shaped recursion by real
   nominal lowering; non-`@` direct recursion still fails with an updated,
   honest message that points at `opt @T`.
5. The V3 memory chapters in the book describe the resulting language exactly,
   with no leftover pre-rewrite wording (Workstream U).
6. Ownership/borrow checking is legal in `core`; every allocating construct is
   gated to `memo`+ with the existing model-legality machinery.
7. Every checked-in `.fol` source intended to be syntactically valid (examples,
   build files, bundled std, showcases) still parses with **zero** tree-sitter
   ERROR nodes; fixtures that pin deleted syntax retain an expected ERROR node.
   The linked-list and tree examples build, run, and free correctly (verified in
   the emitted Rust).


# 4. Workstream Q: Shared Prep (Referenced by the Processor Pillar)

This workstream lands **first**, before any V3 milestone. It is a set of
mechanical, repo-wide renames and deletions plus the ownership diagnostic
family. The processor pillar depends on Q (keyword hygiene, the `name[options]:`
parameter grammar seam, and the sigil charter).

## Q1. Rename the shipped keyword `defer` -> `dfr` everywhere

Legacy policy: the old spelling is **deleted**, with no alias, no compatibility
parse path, no deprecation warning.

Work:

- lexer: rename the `defer` keyword entry to `dfr`
  (`fol-lexer/src/token/buildin/mod.rs:11` and the `BUILDIN::Defer => "defer"`
  display at `:127`); rename the `BUILDIN::Defer` variant if it is user-facing
- parser: rename the `defer`-block production and any `defer`-named AST/helper
  that surfaces to users; keep the internal reverse-order scope-exit semantics
  unchanged
- tree-sitter grammar + queries: rename the `defer` keyword token and every
  highlight/locals/symbols reference
- book: rewrite `book/src/700_sugar/250_dfr.md` to `dfr` (see U); update every
  other chapter that mentions `defer`
- examples: rewrite every `.fol` example using `defer`
- keyword-completion surface: replace `defer` with `dfr` in the LSP keyword
  completion list
- tests: update every pinned `defer` string across `test/` and member crates

## Q2. Remove the unused reserved keyword `go`

Work:

- lexer: delete the `go` keyword entry and the `BUILDIN::Go => "go"` display
  (`buildin/mod.rs:11,138`)
- tree-sitter + LSP keyword completion: remove `go`
- tests: drop any test that pins `go` as a reserved word
- confirm nothing in the parser referenced `BUILDIN::Go`

## Q3. Enforce the sigil charter

Work:

- keep `~var` prefix as sugar for `var[mut]` (already parses); make `var[~]`
  **fail to parse** by removing the `"~"` arm from `binding_option_parsers.rs:31`
- keep `@var` prefix (`VarOption::New`) and `&`/`*` unary operators as-is; their
  full semantics arrive in R and T
- reserve `#` (Hash) as the borrow-from prefix operator and `!` (Bang) prefix as
  give-back — the operators themselves are implemented in S, but Q records the
  charter and resolves the `!` collision: the `[!]` static-var sigil keeps its
  binding-option meaning, and the give-back `!x` is an expression-statement
  prefix disambiguated by position (S owns the grammar)
- delete the `ALL_CAPS`-means-borrowable rule: replace every
  `is_borrowable: name.chars().all(...uppercase...)` computation in
  `routine_header_parsers.rs` and `pipe_lambda_parsers.rs` with `false`; the
  field is retriggered only by `name[bor]:` in S

## Q4. Add the `O####` OWNERSHIP diagnostic family

All move/borrow violations in R, S, and T use `O` codes; the processor pillar
reuses this family for move-at-boundary and Rc-crossing-spawn violations.

Work:

- `fol-diagnostics/src/explain.rs`: add `Some(b'O') => ("OWNERSHIP", "...")` to
  `family_for_code`
- add the pretty-renderer chip/label for the OWNERSHIP family wherever the family
  chip is rendered
- add real `O####` registry entries as each violation is implemented (use-after-
  move, second-mutable-borrow, owner-inaccessible-while-borrowed, etc.)
- extend the registry-honesty test expectations so the new codes are covered and
  the `O` prefix resolves to a real family (guarding
  `every_registered_code_has_a_recognized_family_prefix`)

## Q5. `name[options]: type` parameter-option grammar seam

The `[bor]` and (processor pillar) `[mux]` parameter options need a real
option-bracket position on parameters. Q lands the **grammar seam** only; S and
the processor pillar attach semantics.

Work:

- parser: extend the parameter-name parser (`routine_header_parsers.rs`) to
  accept an optional `[ opt, ... ]` list after the parameter name, before the
  `:` type; parse the options into the existing `Parameter` fields
- reject unknown options at parse time with a precise message
- tree-sitter: add the parameter-option bracket to the parameter rule

Editor / tree-sitter for Q:

- LSP keyword completion: `dfr` present, `defer`/`go` absent
- tree-sitter: keyword renames validated by parsing every checked-in `.fol` with
  zero ERROR nodes
- fixtures using `defer` are rewritten to `dfr`

Docs for Q: see Workstream U (the `dfr` chapter rewrite and the sigil charter
note).

Tracked slices:

- [x] Q1. Rename `defer` -> `dfr` across lexer, parser, tree-sitter, book,
  examples, completion, and tests.
- [x] Q2. Remove the reserved `go` keyword.
- [x] Q3. Enforce the sigil charter: kill `var[~]`, resolve the `!` prefix
  collision, delete the `ALL_CAPS` borrowable hook.
- [x] Q4. Add the `O####` OWNERSHIP diagnostic family, chip, registry, and test
  coverage.
- [x] Q5. Land the `name[options]: type` parameter-option grammar seam.


# 5. Workstream R: M1 — Ownership, Move Semantics, and Nominal Recursive Types

Goal: assignment gains real ownership semantics, and the **flagship** M1
deliverable — heap-recursive types such as a linked list and a tree — builds,
runs, and frees correctly.

## R1. Assignment semantics

Rule:

- **clone-safe** values **clone** on assignment/rebinding — both source and
  destination stay usable
- values with **unique ownership** (including `var[new]` / `@var`, unique
  pointers, and aggregates containing either) **move** — the source becomes
  unusable; any later use is a compile error (`O` code, use-after-move)
- a unique heap value is freed exactly once when its current owner's scope
  ends; the backend lowers a unique heap value to `Box<T>`

Work:

- typecheck: model an ownership state per binding on a scope stack; on
  rebinding, decide clone vs move from the compiler-owned recursive type
  classification; mark the source moved-out on move; emit an `O` use-after-move
  diagnostic on any read of a moved-out binding; delete the heap/new rejection
  at `decls.rs:3471`
- lowering: emit `Box<T>` for unique heap bindings; emit the move as a Rust move
  (ownership transfer) and the clone as a `.clone()`; free happens at scope end
  via ordinary Rust drop of the `Box`
- backend: verify no leaks by inspecting the emitted Rust ownership (unique heap
  = single owner, dropped at scope end)

## R2. Nominal lowered types (the enabling change)

The purely structural `LoweredType` cannot represent self-reference. M1 adds a
**named/nominal** lowered type declaration so `Node` can refer to itself.

Work:

- `fol-lower/src/types.rs`: add a nominal variant to `LoweredType` (a named
  reference to a lowered type declaration) so a record field can name the
  enclosing type instead of inlining it; wire it through `LoweredTypeTable`
  canonicalization
- `fol-lower/src/model.rs`: emit a named lowered type declaration for each `typ`
  so the backend produces one Rust `struct`/`enum` definition and refers to it by
  name at use sites
- backend: emit `Option<Box<Node>>` for `opt @Node`; emit the named struct once

## R3. Recursive types (flagship)

Canonical spelling:

```fol
typ Node: rec = {
    value: int,
    next: opt @Node,
};
```

`next` is an **owned heap child**; the backend lowers it to `Option<Box<Node>>`.

Work:

- typecheck: **replace** `reject_recursive_type_definition`
  (`decls.rs:2527`) for the `@`-shaped case — a self-reference reached only
  through `opt @T` (owned heap) is now legal, because R2 gives it a finite
  lowered shape; a self-reference reached through a **non-`@`** field (a bare
  `Node`, an inlined container of `Node`) stays rejected, with the message
  rewritten to point the user at `opt @T`
- update `checked_type_references_symbol` so the walk distinguishes an
  `@`-guarded back-reference (legal) from a value back-reference (rejected)
- verify tree/linked-list shapes with multiple recursive fields

## R4. Model tiers

- ownership/move checking is **compile-time** and legal in **all** tiers,
  including `core`
- anything that allocates — `[new]` / `@` / recursive `@` fields — requires
  `memo`+, enforced in `fol-typecheck/src/model.rs` exactly like the existing
  `str`/`vec` heap gates
- a `core` program that uses `@` fails with the existing model-legality
  diagnostic shape

Editor / tree-sitter for R:

- tree-sitter: add `@` in **type** position to the type grammar
  (`opt @Node`); add a highlight for the `@` type sigil; validate every
  checked-in `.fol` parses with zero ERROR nodes
- LSP hover on a moved-out binding explains the move; hover on `opt @Node` shows
  the owned-heap-child reading
- LSP diagnostics surface the `O` use-after-move at the use site, citing the move
  site
- LSP go-to-definition on `@Node` resolves to the `Node` declaration

Examples:

- positive: `examples/mem_linked_list_m1` — build a linked list of `@Node`,
  consume its next edge during traversal without cloning ownership, and let it
  free at scope end
- positive: `examples/mem_tree_m1` — build a binary tree with two `opt @Node`
  children, then consume one child edge without duplicating either owner
- positive: `examples/mem_move_stack_vs_heap_m1` — a stack value cloned, a heap
  value moved, both observable
- negative: `examples/fail_mem_use_after_move_m1` — read a heap binding after it
  is moved (`O` code)
- negative: `examples/fail_mem_recursive_value_m1` — `typ Bad: rec = { next: Bad }`
  (non-`@` direct recursion), rejected with the rewritten message
- negative: `examples/fail_mem_heap_in_core_m1` — `@` in a `core` build (tier)

Docs for R: see Workstream U (ownership chapter rewrite; the boundary-note edits
in `docs/v2-generics-m1.md` and `book/src/500_items/500_generics.md`).

Tracked slices:

- [x] R1. Move/clone assignment semantics + use-after-move `O` diagnostic;
  delete the heap-binding rejection.
- [x] R2. Nominal `LoweredType` variant + named lowered type declarations +
  `Box<T>` / `Option<Box<Node>>` backend emission.
- [x] R3. Recursive-type flagship: replace the rejection for `@`-recursion,
  keep + rewrite the non-`@` rejection, update the detector and its pinned
  tests (`test/typecheck` foundation pins, `test/resolver`, and the
  `examples/fail_generic_recursive_m1m2` boundary example).
- [x] R4. Model-tier gating: ownership legal in `core`, allocation gated to
  `memo`+.
- [x] R5. LSP + tree-sitter: `@` in type position, hover/definition/diagnostics.
- [x] R6. Positive (linked list, tree, move-vs-clone) and `fail_*`
  (use-after-move, non-`@` recursion, heap-in-core) examples build/run/reject as
  specified.


# 6. Workstream S: M2 — Borrowing

Goal: scope-granular borrowing with early give-back and mutable-borrow
exclusivity, plus ownership-aware `dfr` and error-only `edf`.

## S1. Enforcement model (scope-granular, not flow-sensitive)

Fixed rules:

- a borrow lives to the **end of the lexical scope** in which it is created
- while a value is borrowed, the **owner is inaccessible** (read of the owner is
  an `O`-code error)
- **one** mutable borrower per scope; a second mutable borrow of the same owner
  in the same scope is an `O`-code error
- a mutable borrow requires the owner declared `var[mut]` **and** the borrower
  declared `var[mut, bor]`
- immutable borrowing is read-only; clone-safe values may be observed, but a
  move-only value cannot be transferred out of any borrow
- a mutable borrower may update the owner, but still cannot transfer unique
  ownership out of the borrow

This is a scope-stack discipline in typecheck. No NLL, no dataflow liveness.
Flow-sensitivity is an explicitly noted **possible future tightening**, and a
**non-goal** for V3 (see Non-Goals).

Work:

- typecheck: extend the R ownership scope-stack with borrow state per owner
  (borrowed / mutably-borrowed / free); enter/leave adjusts the stack at lexical
  scope boundaries; emit `O` diagnostics for owner-access-while-borrowed and
  second-mutable-borrow; delete the `[bor]` rejection (`decls.rs:3466`)

## S2. Borrow bindings and expression sugar

Surface:

- `var[bor] b = owner;` — a borrow binding
- `#owner` — borrow-from expression sugar producing a borrow (new prefix
  operator, per the charter)
- `!b` — early give-back: returns ownership/borrow to the original owner before
  scope end, re-enabling owner access (new prefix operator, per the charter)

Work:

- parser: implement the `#` borrow-from prefix operator and the `!` give-back
  prefix operator (grammar seam reserved in Q3); `#owner` and `!b` become
  expression forms with new `UnaryOperator` variants (`BorrowFrom`, `GiveBack`)
- typecheck: `#owner` yields a borrow of `owner`; `!b` ends the borrow early and
  restores owner access on the scope stack
- lowering/backend: a borrow lowers to a Rust shared/mutable reference with a
  lifetime bounded by the scope; give-back lowers to dropping the reference early

## S3. Borrow parameters

Surface: `name[bor]: T` — a parameter that borrows its argument.

Work:

- parser: attach `[bor]` semantics to the `name[options]:` seam from Q5; set
  `Parameter.is_borrowable = true` from the explicit option (never from casing);
  delete the borrowable-param rejection (`decls.rs:3455`)
- typecheck: a `[bor]` parameter requires either explicit `#owner` at the call
  site or an existing compatible borrow binding; a call-site borrow makes the
  caller's owner inaccessible across the call and restores it on return, while
  an existing borrow keeps its surrounding lexical lifetime and may be reused
- reject returning or forwarding a move-only value from the borrowed parameter
  as an owned value
- lowering/backend: emit a Rust reference parameter

## S4. Ownership-aware `dfr` and error-only `edf`

- `dfr { }` blocks run **before** the scope frees its owned values on
  fallthrough and `break`, and before `return`, recoverable `report`, or `panic`
  exits the scope; a value that moved out is **not** double-freed
- at each exited scope, eligible blocks run in reverse registration order
  **first**, then still-owned heap values in that scope are freed
- `edf { }` is error-only cleanup: it runs **only** for a recoverable `report`
  exit, not for normal fallthrough, `break`, `return`, or panic
- bodies cannot contain `return`, `break`, `report`, or `panic`; cleanup cannot
  initiate another exit while the surrounding exit is being replayed; delayed
  owner reinitialization, channel endpoint acquisition, mutex field access,
  mutex lock/unlock, and forwarding a mutex to another `[mux]` parameter are
  rejected

Work:

- parser: add the `edf { }` block form alongside `dfr { }`
- typecheck: `edf` legal wherever `dfr` is; enforce the delayed-body boundaries
  above and verify interaction with recoverable error paths
- lowering: emit `dfr` before every modeled scope exit and `edf` on recoverable
  error exits only; ensure moved-out values are excluded from the scope's free
  set

Editor / tree-sitter for S:

- tree-sitter: add the `#` and `!` prefix operators, the `[bor]` parameter
  option, and the `edf` block; add highlights for each; validate zero ERROR nodes
- LSP hover on a borrow binding shows "borrow of <owner>"; hover on the owner
  while borrowed shows the inaccessible state; hover on `edf` marks it error-only
- LSP rename covers borrow bindings and `[bor]` parameters (reuse the existing
  local-binding rename path from the local-origins work)
- LSP diagnostics cite the owner and the borrow site for exclusivity failures
- semantic tokens for the borrow sigils if it improves readability

Examples:

- positive: `examples/mem_borrow_m2` — borrow, read, scope-end auto-return
- positive: `examples/mem_borrow_giveback_m2` — `!b` early give-back re-enables
  the owner
- positive: `examples/mem_borrow_param_m2` — a `name[bor]: T` routine
- positive: `examples/mem_mut_borrow_m2` — a `var[mut, bor]` mutable borrow
- positive: `examples/mem_edf_m2` — `edf` runs on recoverable `report`, skipped
  on success
- negative: `examples/fail_mem_owner_while_borrowed_m2` (`O` code)
- negative: `examples/fail_mem_second_mut_borrow_m2` (`O` code)
- negative: `examples/fail_mem_mut_borrow_immutable_owner_m2` (`O` code)

Docs for S: see Workstream U (borrowing rewrite, `dfr`/`edf` chapter section).

Tracked slices:

- [x] S1. Scope-granular borrow state on the ownership scope-stack; owner-
  inaccessible and second-mutable-borrow `O` diagnostics; delete `[bor]`
  rejection.
- [x] S2. `#` borrow-from and `!` give-back prefix operators (parser, typecheck,
  lowering).
- [x] S3. `name[bor]: T` borrow parameters via the Q5 seam; delete borrowable-
  param rejection; retrigger `is_borrowable` from explicit syntax only.
- [x] S4. Ownership-aware `dfr` ordering and error-only `edf` (parser,
  typecheck, lowering) with the explicit exit/body matrix and double-free
  avoidance for moved-out values.
- [x] S5. LSP + tree-sitter: borrow sigils, `[bor]` param, `edf`; hover, rename,
  diagnostics.
- [x] S6. Positive and `fail_*` borrow examples build/run/reject as specified.


# 7. Workstream T: M3 — Pointers

Goal: typed pointers with unique and shared qualifiers, address-of and deref,
and shared heap recursion — with raw pointers explicitly out.

## T1. Typed pointers with a qualifier

Surface:

- `ptr[T]` — unique pointer; backend `Box<T>` (or a unique-owner reference model
  consistent with R's `Box<T>`)
- `ptr[shared, T]` — refcounted shared pointer; backend `Rc<T>`
- `ptr[raw, T]` — **OUT**: deferred to V4/FFI

Work:

- parser: extend `ptr[...]` (`special_type_parsers.rs:183`) to accept an optional
  leading qualifier (`shared` / `raw`) plus the element type, i.e. one or two
  arguments; at the planning baseline it accepted exactly one and errored
  otherwise
- AST: give `FolType::Pointer` a qualifier enum (`Unique` / `Shared` / `Raw`)
  alongside `target` (`ast/types.rs:134`)
- typecheck: delete the pointer-type rejection (`decls.rs:3288`) for `Unique` and
  `Shared`; **keep** a rejection for `Raw` with an honest permanent message
  ("raw pointers are a V4 interop surface"); the `.free()` intrinsic stays
  deferred with `Raw`
- lowering/backend: `Unique` -> `Box<T>`; `Shared` -> `Rc<T>`

## T2. Address-of and deref

Surface: `&x` address-of; `*p` is a by-value dereference or a write-through
place when it is the target of assignment.

Work:

- typecheck: delete the pointer-operator rejection (`exprs/operators.rs:345`);
  type `&x` as `ptr[T]` over `x`'s type; constructing from a move-only `x`
  transfers it into the allocation
- dereference a clone-safe pointee by cloning it and leaving the pointer usable
- dereference a move-only pointee through a direct unique pointer by
  transferring the pointee and consuming that pointer
- reject a move-only pointee read through `ptr[shared, T]` or a borrowed
  pointer; those dereferences are read-only and require clone-safe `T`
- allow write-through only for a direct `var[mut]` unique-pointer binding;
  shared and borrowed pointers are read-only
- reject dereferencing a unique pointer reached through a record field until
  lowering has place-aware projection IR, even when its pointee is clone-safe
- shared deref: the book's double-deref (`*(*pointerPoint)`) is **simplified** —
  the shipped rule is that a single `*p` on a `ptr[shared, T]` yields `T`
  directly when `T` is clone-safe (the refcount indirection is invisible), as
  recorded by the book edit in U
- lowering/backend: `&x` emits `Box::new` / `Rc::new`; clone-safe dereference
  emits an observational clone, while consuming unique dereference transfers
  the value out of its `Box`

## T3. Shared recursion and its boundary

- `opt ptr[shared, Node]` becomes legal at M3 (shared recursive graphs), with a
  **documented boundary**: reference cycles **leak** until weak references exist
- weak references are an explicit **post-M3 / V4 non-goal slot** — named, not
  built
- `Rc` must never cross a spawn boundary; this is enforced in the processor
  pillar (P1) and noted here for cross-reference

Work:

- typecheck: allow `Node` self-reference through `ptr[shared, Node]` (reuses R2's
  nominal lowering); document the leak boundary in diagnostics/hover where useful
- backend: verify `Rc<Node>` graphs compile and run; verify the cycle-leak
  boundary is real (no cycle collector)

## T4. Model tiers

- `ptr[T]` and `ptr[shared, T]` allocate, so both require `memo`+
- pointer typing itself is compile-time and legal to reason about in `core`, but
  constructing one is `memo`+ gated

Editor / tree-sitter for T:

- tree-sitter: extend the `ptr[...]` type rule to accept the qualifier; add
  highlights for `shared` / `raw` qualifiers; `&`/`*` already parse — verify
  their node shape; validate zero ERROR nodes
- LSP hover on `ptr[shared, T]` shows the shared/refcount reading and the leak
  boundary; hover on `*p` shows the pointee type
- LSP diagnostics cite `Raw` as a V4 boundary, not a temporary limitation

Examples:

- positive: `examples/mem_ptr_unique_m3` — consume a `ptr[ptr[int]]` pointee,
  then write/read through the extracted `ptr[int]`
- positive: `examples/mem_ptr_shared_m3` — two `ptr[shared, T]` to one value
- positive: `examples/mem_ptr_shared_recursive_m3` — an `opt ptr[shared, Node]`
  graph
- negative: `examples/fail_mem_ptr_raw_m3` — `ptr[raw, T]` rejected (V4 boundary)
- negative: `examples/fail_mem_ptr_in_core_m3` — pointer construction in `core`
  (tier)
- negative: `examples/fail_mem_pointer_field_deref_m3` — unique-pointer field
  dereference rejected until place-aware projection IR exists
- negative: `examples/fail_mem_shared_ptr_move_deref_m3` — shared pointer cannot
  surrender a move-only pointee
- negative: `examples/fail_mem_borrowed_ptr_move_deref_m3` — borrowed pointer
  cannot surrender a move-only pointee

Docs for T: see Workstream U (pointer chapter rewrite: typed `ptr[T]`, qualifier
form, consuming unique deref, clone-only shared/borrowed deref, raw-out, and
`#`/`&`/`*` alignment).

Tracked slices:

- [x] T1. `ptr[T]` / `ptr[shared, T]` parse + qualifier enum on
  `FolType::Pointer`; delete the pointer-type rejection for unique/shared; keep a
  permanent `Raw` V4 boundary.
- [x] T2. `&x` / `*p` typing, consuming unique dereference, read-only
  shared/borrowed dereference, write-through boundaries, and simplified shared
  deref; delete the pointer-operator rejection.
- [x] T3. Shared recursion legal with a documented cycle-leak boundary; weak-ref
  slot left explicit.
- [x] T4. Model-tier gating for pointer construction.
- [x] T5. LSP + tree-sitter: qualifier grammar, hover, diagnostics.
- [x] T6. Positive (unique, shared, shared-recursive) and `fail_*` (raw, core,
  field projection, shared move-only dereference, borrowed move-only
  dereference) examples build/run/reject as specified.


# 8. Workstream U: Book Updates Required (Memory Pillar)

At planning time the V3 memory chapters were future-design sketches that
contradicted the decisions above in many places. This workstream rewrote them to
match in the same change set as the milestone that owned each fact. Nothing here
was optional prose polish; these were honesty fixes.

Contradictions to fix (chapter -> exact edit):

- `book/src/700_sugar/250_dfr.md`
  - rename `defer` -> `dfr` throughout (Q1)
  - the chapter already lists "ownership-aware cleanup" and "error-only variants
    such as `errdefer`" as later work; replace `errdefer` with the shipped
    `edf` spelling and promote ownership-aware `dfr` + `edf` from "later" to the
    V3 memory contract, with the precise ordering from S4
  - document the actual exit matrix and body boundaries: `dfr` runs for
    fallthrough, `break`, `return`, `report`, and panic; `edf` runs only for a
    recoverable report; return/break/report/panic inside either body, delayed
    owner reinitialization, channel endpoint acquisition, and mutex effects are
    rejected
- `book/src/800_memory/100_ownership.md`
  - all `.echo(...)` / `defer` spellings updated (`dfr`)
  - the chapter says heap uses `[new]` or `[@]`; standardize on `[new]` binding
    and the `@var` / `@T` sigil; drop the `[@]` option spelling
  - the destruction wording (".de_alloc()", ".give_back()") must be replaced:
    freeing is implicit at scope end (Rust `Box` drop), give-back is `!x` only,
    and there is no user-visible `.de_alloc()`
  - the mutable-borrow rule ("only one borrower within one scope") is kept but
    restated as the exact scope-granular rule (owner `var[mut]` + borrower
    `var[mut, bor]`, one mutable borrower per scope)
  - remove any implication of flow-sensitive/NLL borrow checking
- `book/src/800_memory/200_pointers.md`
  - `ptr[]` untyped examples become typed `ptr[T]`
  - the shared-pointer double-deref (`*(*pointerPoint)`) is replaced with the
    simplified single-`*` shared deref rule for clone-safe pointees (T2)
  - the whole **Raw pointer** section is marked a **V4/FFI** boundary, not V3;
    the manual-delete `!(pointerPoint)` examples and `.free()`-style deletion are
    removed from the V3 surface (note: `!x` is give-back, never manual free)
  - `.pointer_value(...)`, `.address_of(...)`, `.borrow_from(...)` intrinsic
    spellings are replaced by the sigils `*p`, `&x`, `#x`
  - document the exact by-value dereference matrix: clone-safe pointees clone;
    move-only pointees transfer only through a direct unique pointer and
    consume it; shared and borrowed pointers require clone-safe pointees
  - document direct mutable unique-pointer write-through and the place-aware
    projection boundary for unique-pointer record fields
  - document the shared-recursion cycle-leak boundary and the weak-ref future
    slot
- boundary docs that describe the recursive-type rejection:
  - `docs/v2-generics-m1.md` and `book/src/500_items/500_generics.md` — update
    the "recursive types are rejected" boundary note to say `@`-recursion is now
    legal via owned heap children, while non-`@` value recursion stays rejected
- the `ALL_CAPS`-borrowable convention: remove it from any chapter that describes
  parameters as borrowable by casing; borrow parameters are `name[bor]:` only
- `plan/VERSIONS.md` — the V3 section is expanded to describe the landed memory
  subset (additive; see the cross-cutting VERSIONS edit shared with the processor
  pillar)

Tracked slices:

- [x] U1. Rewrite `250_dfr.md` (`dfr`, `edf`, ownership-aware ordering, exit
  matrix, and delayed-body limits).
- [x] U2. Rewrite `100_ownership.md` (sigils, implicit free, scope-granular
  borrow rule, no NLL).
- [x] U3. Rewrite `200_pointers.md` (typed `ptr[T]`, qualifier, consuming unique
  deref, clone-only shared/borrowed deref, raw -> V4, sigil intrinsics, leak
  boundary).
- [x] U4. Update the recursive-type boundary notes in `docs/v2-generics-m1.md`
  and `book/src/500_items/500_generics.md`.
- [x] U5. Remove the `ALL_CAPS`-borrowable convention from the book.


# 9. Workstream V: Tooling and Editor Hardening (Cross-Cutting)

This is not a phase after Q through U. It runs **in the same change set** as each
workstream above.

Per-feature editor requirements:

- positive LSP test: open each new positive example, run hover and
  go-to-definition over the new memory surface, assert sensible results
- negative LSP test: open each new `fail_*` example, assert the diagnostic text
  matches the new `O`-code wording (or the tier/boundary message)
- tree-sitter parse test: each new positive example parses to the expected node
  shape with zero ERROR nodes
- tree-sitter parse test: each removed/dead surface (`defer`, `go`, `var[~]`,
  the `ALL_CAPS` borrowable convention, `ptr[raw,...]` as a V3 surface) is not
  silently accepted
- formatter test: every positive memory example remains idempotent and
  compiler-analyzable after formatting; comments and raw strings containing
  memory sigils do not affect structural formatting
- tool-command test: `fol tool parse`, `highlight`, and `symbols` execute the
  generated parser and shipped queries rather than approximating V3 syntax
- inventory test: the canonical positive/failure matrix stays identical to the
  checked-in `mem_*` and `fail_mem_*` package directories
- capability-routing test: direct and routed editor/frontend analysis use the
  evaluated artifact model and preserve the `core` allocation boundary

Primary files:

- `lang/tooling/fol-editor/tree-sitter/grammar.js`
- `lang/tooling/fol-editor/queries/fol/highlights.scm`
- `lang/tooling/fol-editor/queries/fol/locals.scm`
- `lang/tooling/fol-editor/queries/fol/symbols.scm`
- `lang/tooling/fol-editor/src/tree_sitter.rs`
- `lang/tooling/fol-editor/src/lsp/tests/example_models.rs`
- `lang/tooling/fol-editor/src/lsp/tests/navigation.rs`
- `lang/tooling/fol-frontend/src/`
- `lang/compiler/fol-diagnostics/src/`
- `test/v3_example_inventory.rs`
- any fixtures under `lang/tooling/fol-editor/tests/`

Tracked slices:

- [x] V1. Editor / tree-sitter updates shipped alongside Q (renames, dead-form
  removal, parameter-option seam).
- [x] V2. Editor / tree-sitter updates shipped alongside R (`@` in type
  position, move/hover/diagnostics).
- [x] V3. Editor / tree-sitter updates shipped alongside S (`#`/`!` sigils,
  `[bor]` param, `edf`, borrow hover/rename).
- [x] V4. Editor / tree-sitter updates shipped alongside T (pointer qualifier
  grammar, pointer hover/diagnostics).

Rule:

- V is listed separately only for plan visibility
- do not use it as an excuse to ship compiler changes first and editor changes
  later


# 10. Recommended Order

1. Workstream Q (shared prep). Must land first; the processor pillar depends on
   it.
2. Workstream R (M1: ownership, move semantics, nominal recursive types).
   Flagship; unblocks everything else in the memory pillar.
3. Workstream S (M2: borrowing). Builds directly on R's scope-stack.
4. Workstream T (M3: pointers). Reuses R's nominal lowering and R/S ownership
   state.

Workstream U (book) and Workstream V (editor/tree-sitter) run **alongside** each
of the above in the same change set, never deferred.

The memory pillar completes fully (Q -> R -> S -> T) before the processor pillar
(`plan/V3_PROC.md`) begins.


# 11. Non-Goals Restated

Permanently out of scope for the memory pillar (some are named future slots, not
promises):

- flow-sensitive / non-lexical borrow checking (possible future tightening,
  explicitly not V3)
- garbage collection of any kind
- destructors / RAII user hooks / user drop bodies
- raw pointers (`ptr[raw, T]`), manual `.free()` / `.de_alloc()` (V4/FFI)
- weak references and cycle collection (named post-M3 / V4 slot; shared cycles
  leak until then)
- Arc / thread-safe sharing (decided at the processor pillar's mutex boundary)
- any runtime tag or refcount used to decide move vs clone

If a workstream starts to require one of these, it stops.


# 12. Completion Rule

The memory pillar is complete only when:

- prep (Q) has fully landed: `dfr` everywhere, no `go`, sigil charter enforced,
  no `ALL_CAPS` borrowable hook, `O####` OWNERSHIP family live end to end
- M1 (R), M2 (S), and M3 (T) each ship end to end in parser, resolver,
  typecheck, lowering, runtime/backend, frontend routing, structured
  diagnostics, formatter/tool commands, LSP, tree-sitter grammar/queries/corpus,
  docs, book, and canonical examples, and each landed workspace-green
- no "planned for a future release" message in the compiler refers to a memory
  surface the project has chosen to build; raw pointers and weak references
  remain excluded future features, while deliberate shipped restrictions such
  as place-aware field projection, non-moving borrows, clone-only observations,
  and global-safe top-level types retain explicit boundary diagnostics
- the linked-list and tree examples build, run, and free correctly with no leaks
  or hidden ownership clones (verified in the emitted Rust), and every `fail_*`
  example rejects with the specified diagnostic
- the V3 memory chapters (`100_ownership.md`, `200_pointers.md`, `250_dfr.md`)
  describe the resulting language exactly, with no leftover pre-rewrite wording
- the project can honestly say: "V3 memory is compile-time-only ownership,
  scope-granular non-moving borrows, and typed pointers with consuming unique
  dereference — no GC, no NLL, no raw unsafe surface, and heap only where
  `memo`+ allows it"
