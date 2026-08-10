# text_diff

A line-oriented diff. Two revisions of a release-notes file are aligned through
a longest-common-subsequence, the alignment is printed as an edit script, and
the script is rendered as unified hunks. The same generic core then runs over
words to show one rewritten sentence at token granularity.

```
folc code check test/apps/showcases/text_diff
folc code run   test/apps/showcases/text_diff
```

The run prints the diff and finishes with 26 self-checks that re-derive every
stage from fresh documents and compare it against values worked out by hand.
`main` returns 0 only when all of them agree.

## Modules

| module         | what it holds                                                                                     |
| -------------- | ------------------------------------------------------------------------------------------------- |
| `src/lcs/`     | the `keyed` standard, the generic `match_grid`, the dynamic program, the edit script, and the cursors that map script positions back to line numbers |
| `src/doc/`     | `Line` and `Word` (both conformers of `keyed`), ASCII case folding, the reader closures, and the fixtures |
| `src/hunk/`    | context masking, hunk runs, `@@` headers, unified rendering, and the inline word view              |
| `src/checks/`  | the self-checks, including the identity diff and the empty-left-side edge case                     |

## Generic surface exercised

- a protocol standard (`keyed`) used as a generic constraint, with the
  constraint call `element.key()` dispatched per instantiation
- one generic routine instantiated at two unrelated element types, `doc::Line`
  and `doc::Word`
- generic parameters of routine-value type (`{fun (index: int): T}`), so the
  element read happens at the concrete type inside a closure
- closures that capture their backing vector by move and are handed to a
  generic routine

## Shapes the language forced

Four things here are written the way they are because the straightforward
version does not compile today.

**Sequences reach the generic core as accessor closures, not as `vec[T]`.**
An indexed read out of a `vec[T]` is rejected while `T` is still a type
parameter -- `[cln]items[index]` under a `T: clone` bound reports O1001
"move-only indexed projection cannot be read in V3" -- although the identical
read at `vec[str]` is fine. The read therefore has to be performed inside a
closure built where the element type is already concrete, which is why
`doc::line_reader` exists and why every stage asks `doc::before()` for a fresh
document: the closure consumes its vector.

**Both matrices are strings.** FOL containers are fixed at construction: there
is no append and no `table[i] = x`. A DP table whose size is only known at run
time cannot be a `vec[int]`, so each cell is stored as a zero-padded three-digit
decimal inside a `str` that grows by concatenation. The same reason keeps the
edit script a string of opcodes and the hunk mask a string of flags -- which
turned out to suit the domain, since an edit script is a natural artifact to
print and to compare in a test.

**Opcodes are routines, not constants.** `con OP_KEEP: str = "="` type-checks
and then makes the backend emit a Rust `char` where a `FolStr` is expected, so
the build fails after `check` has already passed. A `fun[exp] op_keep(): str`
lowers correctly. Naming them at all is not optional either: a one-character
literal with no expected type infers as `chr`, and `chr` will not compare
against `str`.

**`match_grid` binds each key to `str` immediately.** A local of the parameter
type `T` that stays live across a branch inside a loop makes the backend emit a
`drop` on a path where the value was never reinitialised, which rustc rejects.
Reading the key straight out of the accessor call avoids holding a `T` at all.

Two smaller notes: names fold case- and underscore-insensitively, so the cell
width constant is `WIDTH` rather than `CELL` (which would collide with the
routine `cell`), and the parameters are `pairs`/`costs` rather than
`grid`/`table` for the same reason.

## What the tool reports

```
edit script
  =+++===--+======++
  common lines: 10 of 12
```

Twelve old lines and sixteen new ones share a ten-line subsequence. The
reworded bullet cannot be matched, and taking it out of the alignment costs its
neighbour too, so two lines are dropped and eight are added across two hunks.
